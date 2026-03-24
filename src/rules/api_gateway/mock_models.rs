use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: flags `jest.mock("#models", ...)` calls in index.spec.js files.
/// Mocking #models in tests hides real database behavior and leads to
/// brittle tests that pass while production breaks.
pub struct MockModels;

const SCORE_PER_VIOLATION: u32 = 100;

impl Rule for MockModels {
    fn name(&self) -> &str {
        "mock-models"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/mock-models.md"
    }

    fn description(&self) -> &str {
        "Flags jest.mock(\"#models\", ...) in index.spec.js files"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["jest.mock(\"#models\", () => ({ User: { findOne: jest.fn() } }));"],
            &["// Use real models in tests instead of mocking #models"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        // Only apply to index.spec.js files
        if !path.file_name().is_some_and(|f| f == "index.spec.js") {
            return vec![];
        }

        let score = config.rule_score("mock-models", SCORE_PER_VIOLATION);

        let mut violations = Vec::new();
        collect_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Walk the AST looking for jest.mock("#models", ...) calls.
fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &MockModels,
    score: u32,
) {
    if node.kind() == "call_expression" {
        if let Some(callee) = node.child_by_field_name("function") {
            if callee.kind() == "member_expression" {
                let obj = callee.child_by_field_name("object");
                let prop = callee.child_by_field_name("property");
                if obj.is_some_and(|o| &source[o.byte_range()] == "jest")
                    && prop.is_some_and(|p| &source[p.byte_range()] == "mock")
                {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        let mut cursor = args.walk();
                        for arg in args.children(&mut cursor) {
                            if arg.kind() == "string" {
                                let text = &source[arg.byte_range()];
                                if text == "\"#models\"" || text == "'#models'" {
                                    let line = source.lines().nth(node.start_position().row).unwrap_or("");
                                    violations.push(RuleViolation {
                                        rule_name: rule.name().to_string(),
                                        doc_url: rule.doc_url().to_string(),
                                        score,
                                        code_sample: line.trim().to_string(),
                                        fix_suggestion: "use real models instead of mocking #models".to_string(),
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }
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
        MockModels.check(source, Path::new("src/services/user/index.spec.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        MockModels.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_jest_mock_models_double_quotes() {
        let violations = check(r##"jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));"##);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_jest_mock_models_single_quotes() {
        let violations = check("jest.mock('#models', () => ({ User: { findOne: jest.fn() } }));");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_jest_mock_models_multiline() {
        let source = r##"jest.mock("#models", () => ({
    TremendousWebhookEvent: { findOne: jest.fn() },
}));"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_mock_models_calls() {
        let source = r##"
jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));
jest.mock("#models", () => ({ Order: { create: jest.fn() } }));
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 200);
    }

    // --- No violations (wrong file) ---

    #[test]
    fn test_not_triggered_in_regular_js() {
        let violations = check_with_path(
            r##"jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));"##,
            "src/services/user/index.js",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_triggered_in_other_spec() {
        let violations = check_with_path(
            r##"jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));"##,
            "src/services/user/helper.spec.js",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_triggered_in_test_js() {
        let violations = check_with_path(
            r##"jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));"##,
            "src/services/user/test.js",
        );
        assert!(violations.is_empty());
    }

    // --- No violations (different mock targets) ---

    #[test]
    fn test_jest_mock_other_module_not_flagged() {
        let violations = check(r##"jest.mock("#utils", () => ({ helper: jest.fn() }));"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_jest_mock_relative_path_not_flagged() {
        let violations = check(r#"jest.mock("./models", () => ({ User: jest.fn() }));"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_jest_mock_models_substring_not_flagged() {
        let violations = check(r##"jest.mock("#models-v2", () => ({ User: jest.fn() }));"##);
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_jest_mock_models_in_comment() {
        let violations = check(r##"// jest.mock("#models", () => ({}));"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_jest_mock_models_in_string() {
        let violations = check(r##"const x = 'jest.mock("#models")';"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_jest_object() {
        let violations = check(r##"foo.mock("#models", () => ({}));"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_mock_method() {
        let violations = check(r##"jest.fn("#models", () => ({}));"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_jest_mock_no_args() {
        let violations = check("jest.mock();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_nested_index_spec_path() {
        let violations = check_with_path(
            r##"jest.mock("#models", () => ({ User: { findOne: jest.fn() } }));"##,
            "packages/api/src/services/user/index.spec.js",
        );
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(r##"jest.mock("#models", () => ({}));"##);
        assert_eq!(violations[0].rule_name, "mock-models");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(r##"jest.mock("#models", () => ({}));"##);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
