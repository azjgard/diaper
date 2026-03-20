use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: in migration files, using queryInterface.addColumn or
/// queryInterface.removeColumn directly adds 50 stink per call.
/// Use idempotent column helpers from #library/sequelize/migrations/index.js instead.
pub struct NonIdempotentMigration;

const SCORE_PER_VIOLATION: u32 = 50;

impl Rule for NonIdempotentMigration {
    fn name(&self) -> &str {
        "non-idempotent-migration"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/non-idempotent-migration.md"
    }

    fn description(&self) -> &str {
        "addColumn/removeColumn in migrations (non-idempotent)"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["queryInterface.addColumn('users', 'email', ...)"],
            &["// use a raw SQL migration instead"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let path_str = path.to_string_lossy();
        if !path_str.contains("/migrations") {
            return vec![];
        }

        let score = config.rule_score("non-idempotent-migration", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        find_non_idempotent_calls(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

const FLAGGED_METHODS: &[&str] = &["addColumn", "removeColumn"];

/// Walk the AST looking for queryInterface.addColumn or queryInterface.removeColumn calls.
fn find_non_idempotent_calls(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &NonIdempotentMigration,
    score: u32,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let (Some(object), Some(property)) = (
                    func.child_by_field_name("object"),
                    func.child_by_field_name("property"),
                ) {
                    let obj_text = &source[object.byte_range()];
                    let prop_text = &source[property.byte_range()];

                    if obj_text == "queryInterface" && FLAGGED_METHODS.contains(&prop_text) {
                        let line = source.lines().nth(node.start_position().row).unwrap_or("");
                        violations.push(RuleViolation {
                            rule_name: rule.name().to_string(),
                            doc_url: rule.doc_url().to_string(),
                            score,
                            code_sample: line.trim().to_string(),
                            fix_suggestion: format!(
                                "use idempotent column helpers from #library/sequelize/migrations/index.js instead of queryInterface.{prop_text}"
                            ),
                        });
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_non_idempotent_calls(child, source, violations, rule, score);
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
        NonIdempotentMigration.check(source, Path::new("src/migrations/001-add-col.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        NonIdempotentMigration.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_add_column() {
        let violations = check("queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
    }

    #[test]
    fn test_remove_column() {
        let violations = check("queryInterface.removeColumn('users', 'email');");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
    }

    #[test]
    fn test_await_add_column() {
        let violations = check("await queryInterface.addColumn('users', 'name', { type: DataTypes.STRING });");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_await_remove_column() {
        let violations = check("await queryInterface.removeColumn('users', 'name');");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_calls() {
        let source = r#"
await queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });
await queryInterface.addColumn('users', 'name', { type: DataTypes.STRING });
await queryInterface.removeColumn('users', 'old_field');
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 3);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 150);
    }

    #[test]
    fn test_nested_in_function() {
        let source = r#"
module.exports = {
  up: async (queryInterface) => {
    await queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });
  }
};
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- Path exclusions ---

    #[test]
    fn test_non_migration_file_skipped() {
        let violations = check_with_path(
            "queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });",
            "src/services/user.js",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migration_file_detected() {
        let violations = check_with_path(
            "queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });",
            "db/migrations/20230101-add-email.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_nested_migrations_path() {
        let violations = check_with_path(
            "queryInterface.addColumn('users', 'email', { type: DataTypes.STRING });",
            "packages/core/src/migrations/001.js",
        );
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_other_query_interface_methods_ok() {
        let violations = check("queryInterface.createTable('users', {});");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_other_object_add_column_ok() {
        let violations = check("someOtherThing.addColumn('users', 'email');");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_query_interface() {
        let violations = check("const x = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_comment_no_match() {
        let violations = check("// queryInterface.addColumn('users', 'email');");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_string_no_match() {
        let violations = check(r#"const s = "queryInterface.addColumn('users', 'email')";"#);
        assert!(violations.is_empty());
    }

    // --- Fix suggestion ---

    #[test]
    fn test_fix_suggestion_add_column() {
        let violations = check("queryInterface.addColumn('users', 'email', {});");
        assert!(violations[0].fix_suggestion.contains("addColumn"));
        assert!(violations[0].fix_suggestion.contains("#library/sequelize/migrations/index.js"));
    }

    #[test]
    fn test_fix_suggestion_remove_column() {
        let violations = check("queryInterface.removeColumn('users', 'email');");
        assert!(violations[0].fix_suggestion.contains("removeColumn"));
        assert!(violations[0].fix_suggestion.contains("#library/sequelize/migrations/index.js"));
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("queryInterface.addColumn('users', 'email', {});");
        assert_eq!(violations[0].rule_name, "non-idempotent-migration");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("queryInterface.addColumn('users', 'email', {});");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
