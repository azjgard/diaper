use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: flags `new Date()` usage. Use dayjs from #library/dayjs instead.
pub struct NewDate;

const SCORE_PER_VIOLATION: u32 = 10;

impl Rule for NewDate {
    fn name(&self) -> &str {
        "new-date"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/new-date.md"
    }

    fn description(&self) -> &str {
        "Flags new Date() usage"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["const now = new Date();"],
            &["import dayjs from \"#library/dayjs\";\nconst now = dayjs();"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("new-date", SCORE_PER_VIOLATION);

        let mut violations = Vec::new();
        collect_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Walk the AST looking for `new Date(...)` expressions.
fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &NewDate,
    score: u32,
) {
    if node.kind() == "new_expression" {
        if let Some(constructor) = node.child_by_field_name("constructor") {
            if &source[constructor.byte_range()] == "Date" {
                let line = source.lines().nth(node.start_position().row).unwrap_or("");
                violations.push(RuleViolation {
                    rule_name: rule.name().to_string(),
                    doc_url: rule.doc_url().to_string(),
                    score,
                    code_sample: line.trim().to_string(),
                    fix_suggestion: "use dayjs from #library/dayjs instead of new Date()".to_string(),
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_violations(child, source, violations, rule, score);
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
        NewDate.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        NewDate.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_new_date_no_args() {
        let violations = check("const now = new Date();");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_new_date_with_args() {
        let violations = check("const d = new Date(2024, 0, 1);");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_new_date_with_string_arg() {
        let violations = check(r#"const d = new Date("2024-01-01");"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_new_date_inline() {
        let violations = check("if (new Date() > deadline) {}");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_new_dates() {
        let source = "const a = new Date();\nconst b = new Date();";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 20);
    }

    // --- No violations ---

    #[test]
    fn test_no_new_date() {
        let violations = check("const now = dayjs();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_date_static_method() {
        let violations = check("const ts = Date.now();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_date_variable_name() {
        let violations = check("const Date = 'something';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_new_other_class() {
        let violations = check("const x = new Map();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    // --- Excluded paths ---

    #[test]
    fn test_excluded_spec_file() {
        let violations = check_with_path("const d = new Date();", "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_migration() {
        let violations = check_with_path("const d = new Date();", "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_new_date_in_comment() {
        let violations = check("// const d = new Date();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_new_date_in_string() {
        let violations = check(r#"const x = "new Date()";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_new_date_in_template_literal() {
        let violations = check("const x = `${new Date()}`;");
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("const d = new Date();");
        assert_eq!(violations[0].rule_name, "new-date");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("const d = new Date();");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
