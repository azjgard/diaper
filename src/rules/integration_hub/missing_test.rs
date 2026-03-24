use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: non-excluded JS files that define functions or run logic must have
/// an index.spec.js test file in the same directory.
pub struct MissingTest;

const SCORE_PER_VIOLATION: u32 = 50;

impl Rule for MissingTest {
    fn name(&self) -> &str {
        "missing-test"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/missing-test.md"
    }

    fn description(&self) -> &str {
        "Files with functions or logic must have an index.spec.js in the same directory"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["// src/handler/index.js with no index.spec.js"],
            &["// src/handler/index.js with sibling index.spec.js"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if is_excluded_path(path) {
            return vec![];
        }

        if is_barrel_or_no_logic(tree.root_node(), source) {
            return vec![];
        }

        let dir = match path.parent() {
            Some(d) => d,
            None => return vec![],
        };

        let spec_path = dir.join("index.spec.js");
        if spec_path.exists() {
            return vec![];
        }

        let score = config.rule_score("missing-test", SCORE_PER_VIOLATION);
        let display_path = path.to_string_lossy();

        vec![RuleViolation {
            rule_name: self.name().to_string(),
            doc_url: self.doc_url().to_string(),
            score,
            code_sample: String::new(),
            fix_suggestion: format!("add tests for {display_path} in an index.spec.js in the same directory"),
        }]
    }
}

/// Returns true if this file path should be excluded from the missing-test rule.
fn is_excluded_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Spec files themselves
    if path_str.ends_with("index.spec.js") {
        return true;
    }

    // Migrations, tests, types directories
    if path_str.contains("/migrations/")
        || path_str.contains("/tests/")
        || path_str.contains("/types/")
    {
        return true;
    }

    false
}

/// Returns true if the file is a barrel file (only re-exports) or defines
/// no functions and runs no logic (just constants/type imports).
fn is_barrel_or_no_logic(root: tree_sitter::Node, source: &str) -> bool {
    let mut has_function = false;
    let mut has_call = false;

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            // Function declarations count as logic
            "function_declaration" | "generator_function_declaration" => {
                has_function = true;
            }
            // export default function / export function
            "export_statement" => {
                if has_function_in_export(child, source) {
                    has_function = true;
                }
                // Bare re-exports like `export { foo } from './foo'` are barrel-like
                // and don't count as logic — we only care if there's a function inside
            }
            // const foo = () => {} or const foo = function() {}
            "lexical_declaration" | "variable_declaration" => {
                if has_function_in_var_decl(child, source) {
                    has_function = true;
                }
            }
            // Top-level call expressions like `module.exports = ...` or `console.log(...)`
            "expression_statement" => {
                if contains_call_expression(child) {
                    has_call = true;
                }
            }
            _ => {}
        }
    }

    !has_function && !has_call
}

/// Check if an export statement contains a function definition.
fn has_function_in_export(node: tree_sitter::Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "function" | "arrow_function" | "generator_function_declaration" => {
                return true;
            }
            "lexical_declaration" | "variable_declaration" => {
                if has_function_in_var_decl(child, source) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if a variable declaration assigns a function/arrow.
fn has_function_in_var_decl(node: tree_sitter::Node, _source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(value) = child.child_by_field_name("value") {
                match value.kind() {
                    "arrow_function" | "function" => return true,
                    _ => {
                        if contains_function_node(value) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a node is or directly contains a function expression.
fn contains_function_node(node: tree_sitter::Node) -> bool {
    match node.kind() {
        "arrow_function" | "function" => true,
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if contains_function_node(child) {
                    return true;
                }
            }
            false
        }
    }
}

/// Check if a node contains a call expression (top-level logic execution).
fn contains_call_expression(node: tree_sitter::Node) -> bool {
    if node.kind() == "call_expression" {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_call_expression(child) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;
    use std::fs;

    fn check_with_dir(source: &str, has_spec: bool) -> Vec<RuleViolation> {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("index.js");
        fs::write(&file_path, source).unwrap();

        if has_spec {
            fs::write(dir.path().join("index.spec.js"), "test('it works', () => {});").unwrap();
        }

        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        MissingTest.check(source, &file_path, &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        MissingTest.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_file_with_function_no_spec() {
        let violations = check_with_dir("function handler() {}", false);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
    }

    #[test]
    fn test_file_with_arrow_function_no_spec() {
        let violations = check_with_dir("const handler = () => {};", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_file_with_exported_function_no_spec() {
        let violations = check_with_dir("export function handler() {}", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_file_with_default_export_function_no_spec() {
        let violations = check_with_dir("export default function handler() {}", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_file_with_default_export_arrow_no_spec() {
        let violations = check_with_dir("export default () => {};", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_file_with_async_function_no_spec() {
        let violations = check_with_dir("export const fetch = async () => {};", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_file_with_toplevel_call_no_spec() {
        let violations = check_with_dir("const x = 1;\nconsole.log(x);", false);
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_file_with_function_has_spec() {
        let violations = check_with_dir("function handler() {}", true);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_barrel_file_only_reexports() {
        let violations = check_with_dir(
            "export { default as foo } from './foo';\nexport { default as bar } from './bar';",
            false,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_constants_only() {
        let violations = check_with_dir("const X = 1;\nconst Y = 'hello';", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_import_and_constants() {
        let violations = check_with_dir(
            "import { FOO } from './constants';\nconst BAR = FOO + 1;",
            false,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check_with_dir("", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_only_comments() {
        let violations = check_with_dir("// this is a comment\n/* block comment */", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_object_no_functions() {
        let violations = check_with_dir("export const config = { port: 3000 };", false);
        assert!(violations.is_empty());
    }

    // --- Excluded paths ---

    #[test]
    fn test_excluded_spec_file() {
        let violations = check_with_path("function test() {}", "src/handler/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_migrations() {
        let violations = check_with_path("function up() {}", "db/migrations/001.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_tests_dir() {
        let violations = check_with_path("function helper() {}", "src/tests/helpers.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_types_dir() {
        let violations = check_with_path("function makeType() {}", "src/types/index.js");
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_function_in_string_not_detected() {
        // A string containing "function" should not trigger — but the file has no real functions
        let violations = check_with_dir(r#"const x = "function foo() {}";"#, false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_function_in_comment_not_detected() {
        let violations = check_with_dir("// function foo() {}", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mixed_constants_and_function() {
        let violations = check_with_dir("const X = 1;\nfunction handler() {}\nconst Y = 2;", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_generator_function_no_spec() {
        let violations = check_with_dir("function* gen() { yield 1; }", false);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_function_expression_in_var() {
        let violations = check_with_dir("const handler = function() {};", false);
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check_with_dir("function handler() {}", false);
        assert_eq!(violations[0].rule_name, "missing-test");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check_with_dir("function handler() {}", false);
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_violation_code_sample_is_empty() {
        let violations = check_with_dir("function handler() {}", false);
        assert!(violations[0].code_sample.is_empty());
    }

    #[test]
    fn test_violation_fix_suggestion_mentions_file() {
        let violations = check_with_dir("function handler() {}", false);
        assert!(violations[0].fix_suggestion.contains("index.spec.js"));
    }

    #[test]
    fn test_single_violation_per_file() {
        // Even with multiple functions, only one violation per file (missing spec)
        let violations = check_with_dir("function a() {}\nfunction b() {}\nconst c = () => {};", false);
        assert_eq!(violations.len(), 1);
    }
}
