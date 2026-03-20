use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{Rule, RuleViolation};

pub struct SqlTableAlias;

const SCORE_PER_VIOLATION: u32 = 100;

/// SQL keywords that should not be treated as table aliases.
const SQL_KEYWORDS: &[&str] = &[
    "ON", "WHERE", "SET", "VALUES", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "CROSS", "FULL",
    "AND", "OR", "ORDER", "GROUP", "HAVING", "LIMIT", "UNION", "AS", "SELECT", "FROM", "INTO",
    "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "IN", "NOT", "NULL", "IS", "LIKE",
    "BETWEEN", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END", "DISTINCT", "ALL", "ANY", "SOME",
];

fn sql_detect_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(SELECT|INSERT|UPDATE|DELETE|FROM|JOIN)\b").unwrap())
}

/// Pattern for a SQL identifier: either a quoted identifier like `"companyUsers"` or a bare word.
const IDENT: &str = r#"(?:"[^"]+"|[\w]+)"#;

fn from_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let pattern = format!(
            r#"(?i)\bFROM\s+({ident}(?:\.{ident})?)(?:\s+AS\s+({ident})|\s+({ident}))?"#,
            ident = IDENT
        );
        Regex::new(&pattern).unwrap()
    })
}

fn join_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let pattern = format!(
            r#"(?i)\b(?:(?:LEFT|RIGHT|INNER|OUTER|CROSS|FULL)\s+)?JOIN\s+({ident}(?:\.{ident})?)(?:\s+AS\s+({ident})|\s+({ident}))?"#,
            ident = IDENT
        );
        Regex::new(&pattern).unwrap()
    })
}

struct TableRef {
    table_name: String,
    alias: Option<String>,
}

/// Strip surrounding double quotes from a Postgres quoted identifier.
fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)
}

fn is_sql_keyword(word: &str) -> bool {
    let upper = word.to_uppercase();
    SQL_KEYWORDS.iter().any(|kw| *kw == upper)
}

fn looks_like_sql(text: &str) -> bool {
    sql_detect_regex().is_match(text)
}

fn extract_string_text(node: tree_sitter::Node, source: &str) -> String {
    let mut result = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_fragment" | "\"" | "'" | "`" => {
                result.push_str(&source[child.byte_range()]);
            }
            "template_substitution" => {
                result.push_str("placeholder");
            }
            _ => {}
        }
    }
    result
}

fn find_table_refs(sql: &str) -> Vec<TableRef> {
    let mut refs = Vec::new();

    for cap in from_regex().captures_iter(sql) {
        let table_name = strip_quotes(&cap[1]).to_string();
        let alias = cap.get(2).or_else(|| cap.get(3)).map(|m| strip_quotes(m.as_str()).to_string());
        let alias = alias.filter(|a| !is_sql_keyword(a));
        refs.push(TableRef { table_name, alias });
    }

    for cap in join_regex().captures_iter(sql) {
        let table_name = strip_quotes(&cap[1]).to_string();
        let alias = cap.get(2).or_else(|| cap.get(3)).map(|m| strip_quotes(m.as_str()).to_string());
        let alias = alias.filter(|a| !is_sql_keyword(a));
        refs.push(TableRef { table_name, alias });
    }

    refs
}

fn check_sql_for_violations(sql: &str) -> Vec<(String, String)> {
    let refs = find_table_refs(sql);

    // Count how many times each table appears (case-insensitive).
    let mut table_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in &refs {
        // Strip schema prefix for counting (e.g. "public.users" -> "users")
        let base = r.table_name.rsplit('.').next().unwrap_or(&r.table_name);
        *table_counts.entry(base.to_lowercase()).or_insert(0) += 1;
    }

    let mut violations = Vec::new();
    for r in &refs {
        if let Some(alias) = &r.alias {
            let base = r.table_name.rsplit('.').next().unwrap_or(&r.table_name);
            let count = table_counts.get(&base.to_lowercase()).copied().unwrap_or(0);
            if count <= 1 {
                // Table appears once — alias is always a violation.
                violations.push((r.table_name.clone(), alias.clone()));
            } else {
                // Table appears multiple times — violation only if alias doesn't contain table name.
                if !alias.to_lowercase().contains(&base.to_lowercase()) {
                    violations.push((r.table_name.clone(), alias.clone()));
                }
            }
        }
    }

    violations
}

fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &SqlTableAlias,
    score: u32,
) {
    let kind = node.kind();
    if kind == "string" || kind == "template_string" {
        let text = extract_string_text(node, source);
        if looks_like_sql(&text) {
            for (table, alias) in check_sql_for_violations(&text) {
                let line = source.lines().nth(node.start_position().row).unwrap_or("");
                violations.push(RuleViolation {
                    rule_name: rule.name().to_string(),
                    doc_url: rule.doc_url().to_string(),
                    score,
                    code_sample: line.trim().to_string(),
                    fix_suggestion: format!(
                        "Remove the alias `{alias}` from table `{table}` — use the full table name, or if an alias is needed, include the original table name in it (e.g. `sender{table}`)"
                    ),
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_violations(child, source, violations, rule, score);
    }
}

impl Rule for SqlTableAlias {
    fn name(&self) -> &str {
        "sql-table-alias"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/sql-table-alias.md"
    }

    fn check(
        &self,
        source: &str,
        _path: &Path,
        tree: &tree_sitter::Tree,
        _cache: &mut super::AstCache,
        config: &crate::config::Config,
    ) -> Vec<RuleViolation> {
        let score = config.rule_score("sql-table-alias", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        SqlTableAlias.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_from_single_letter_alias() {
        let violations = check(r#"const q = "SELECT * FROM users u WHERE u.active";"#);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_from_alias_with_as_keyword() {
        let violations = check(r#"const q = "SELECT * FROM users AS u WHERE u.active";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_join_alias() {
        let violations = check(r#"const q = "SELECT * FROM users JOIN orders o ON o.user_id = users.id";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_left_join_alias() {
        let violations = check(r#"const q = "SELECT * FROM users LEFT JOIN orders o ON o.user_id = users.id";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_aliases_in_one_query() {
        let violations = check(r#"const q = "SELECT * FROM users u JOIN orders o ON o.user_id = u.id";"#);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_template_literal_with_alias() {
        let violations = check("const q = `SELECT * FROM users u WHERE u.id = ${id}`;");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_single_quoted_string_with_alias() {
        let violations = check("const q = 'SELECT * FROM users u';");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_duplicate_table_non_descriptive_aliases() {
        let violations = check(r#"const q = "SELECT * FROM users x JOIN users y ON x.id = y.id";"#);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_quoted_table_with_alias() {
        let violations = check(r#"const q = "SELECT * FROM \"companyUsers\" cu WHERE cu.active";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_duplicate_quoted_table_non_descriptive_aliases() {
        let violations = check(r#"const q = "SELECT * FROM \"companyUsers\" x JOIN \"companyUsers\" y ON x.id = y.id";"#);
        assert_eq!(violations.len(), 2);
    }

    // --- No violations ---

    #[test]
    fn test_no_alias_from() {
        let violations = check(r#"const q = "SELECT * FROM users WHERE active = true";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_alias_join() {
        let violations = check(r#"const q = "SELECT * FROM users JOIN orders ON orders.user_id = users.id";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_descriptive_alias_on_duplicate_join() {
        let violations = check(
            r#"const q = "SELECT * FROM \"companyUsers\" \"senderCompanyUsers\" JOIN \"companyUsers\" \"recipientCompanyUsers\" ON \"senderCompanyUsers\".id = \"recipientCompanyUsers\".id";"#,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_sql_string() {
        let violations = check(r#"const msg = "Hello world, welcome to the team";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_sql_keyword_not_treated_as_alias() {
        let violations = check(r#"const q = "SELECT * FROM users WHERE active = true";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_from_followed_by_join_not_alias() {
        let violations = check(r#"const q = "SELECT * FROM users JOIN orders ON orders.user_id = users.id";"#);
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_string() {
        let violations = check(r#"const q = "";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_sql_in_comment_not_matched() {
        let violations = check(r#"// SELECT * FROM users u"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_multiline_template_sql() {
        let source = r#"const q = `
            SELECT *
            FROM users u
            WHERE u.active
        `;"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_mixed_case_sql_keywords() {
        let violations = check(r#"const q = "select * from users u where u.active";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_sql_strings_in_one_file() {
        let source = r#"
const q1 = "SELECT * FROM users u";
const q2 = "SELECT * FROM orders o";
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_insert_without_alias() {
        let violations = check(r#"const q = "INSERT INTO users VALUES (1, 'bob')";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_update_without_alias() {
        let violations = check(r#"const q = "UPDATE users SET name = 'bob' WHERE id = 1";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_delete_without_alias() {
        let violations = check(r#"const q = "DELETE FROM users WHERE id = 1";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_schema_qualified_name() {
        let violations = check(r#"const q = "SELECT * FROM public.users u WHERE u.active";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_subquery_not_matched() {
        // Parenthesized subquery — `(SELECT ...)` doesn't match `\w+`
        let violations = check(r#"const q = "SELECT * FROM (SELECT id FROM users) sub";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_right_join_alias() {
        let violations = check(r#"const q = "SELECT * FROM users RIGHT JOIN orders o ON o.user_id = users.id";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_inner_join_alias() {
        let violations = check(r#"const q = "SELECT * FROM users INNER JOIN orders o ON o.user_id = users.id";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_full_outer_join_alias() {
        let violations = check(r#"const q = "SELECT * FROM users FULL OUTER JOIN orders o ON o.user_id = users.id";"#);
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_rule_name() {
        let rule = SqlTableAlias;
        assert_eq!(rule.name(), "sql-table-alias");
    }

    #[test]
    fn test_doc_url_starts_with_https() {
        let rule = SqlTableAlias;
        assert!(rule.doc_url().starts_with("https://"));
    }

    #[test]
    fn test_violation_fix_suggestion() {
        let violations = check(r#"const q = "SELECT * FROM users u";"#);
        assert!(violations[0].fix_suggestion.contains("users"));
        assert!(violations[0].fix_suggestion.contains("`u`"));
        assert!(violations[0].fix_suggestion.contains("include the original table name"));
    }

    #[test]
    fn test_config_score_override() {
        let source = r#"const q = "SELECT * FROM users u";"#;
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let mut config = crate::config::Config::default();
        config.rules.insert(
            "sql-table-alias".to_string(),
            crate::config::RuleConfig::Score(200),
        );
        let violations = SqlTableAlias.check(source, Path::new("test.js"), &tree, &mut cache, &config);
        assert_eq!(violations[0].score, 200);
    }
}
