use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: usage of `async` or `await` keywords adds 100 stink per occurrence,
/// unless the file is named "index.spec.js" or the path contains "/migrations".
pub struct AsyncAwait;

const SCORE_PER_VIOLATION: u32 = 100;

impl Rule for AsyncAwait {
    fn name(&self) -> &str {
        "async-await"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/async-await.md"
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("async-await", SCORE_PER_VIOLATION);

        let mut violations = Vec::new();
        collect_async_await(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Walk the AST and find async functions and await expressions.
fn collect_async_await(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &AsyncAwait,
    score: u32,
) {
    match node.kind() {
        "function_declaration" | "arrow_function" | "function" | "generator_function_declaration" => {
            if node.child_by_field_name("async").is_some() || is_async_node(node, source) {
                let line = source.lines().nth(node.start_position().row).unwrap_or("");
                violations.push(RuleViolation {
                    rule_name: rule.name().to_string(),
                    doc_url: rule.doc_url().to_string(),
                    score,
                    code_sample: line.trim().to_string(),
                    fix_suggestion: "remove async/await and use synchronous patterns or callbacks".to_string(),
                });
            }
        }
        "await_expression" => {
            let line = source.lines().nth(node.start_position().row).unwrap_or("");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion: "remove async/await and use synchronous patterns or callbacks".to_string(),
            });
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_async_await(child, source, violations, rule, score);
    }
}

/// Check if a function node has the "async" keyword by looking at its text.
fn is_async_node(node: tree_sitter::Node, source: &str) -> bool {
    let start = node.start_byte();
    if start + 5 <= source.len() {
        &source[start..start + 5] == "async"
    } else {
        false
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
        AsyncAwait.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        AsyncAwait.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_async_function() {
        let violations = check("async function foo() {}");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_await_expression() {
        let violations = check("const x = await fetch('/api');");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_async_arrow() {
        let violations = check("const foo = async () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_async_and_await_same_line() {
        let violations = check("const x = async () => await fetch('/api');");
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_multiple_lines() {
        let source = "async function foo() {\n  const x = await bar();\n}";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 200);
    }

    #[test]
    fn test_multiple_awaits_same_line() {
        let violations = check("const [a, b] = await Promise.all([await foo(), await bar()]);");
        assert_eq!(violations.len(), 3);
    }

    // --- Excluded paths ---

    #[test]
    fn test_index_spec_js_excluded() {
        let violations = check_with_path("async function foo() {}", "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_index_spec_js_nested_excluded() {
        let violations = check_with_path("async function foo() {}", "packages/app/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_path_excluded() {
        let violations = check_with_path("async function foo() {}", "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_nested_excluded() {
        let violations = check_with_path("async function foo() {}", "db/migrations/seed.js");
        assert!(violations.is_empty());
    }

    // --- NOT excluded ---

    #[test]
    fn test_other_spec_file_not_excluded() {
        let violations = check_with_path("async function foo() {}", "src/foo.spec.js");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_regular_file_not_excluded() {
        let violations = check_with_path("async function foo() {}", "src/service.js");
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_no_async_await() {
        let violations = check("function foo() { return 1; }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_in_variable_name_no_match() {
        let violations = check("const asyncFoo = 1;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_await_in_variable_name_no_match() {
        let violations = check("const awaitResult = 1;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_in_string_no_match() {
        // tree-sitter correctly identifies this as a string, not a keyword
        let violations = check(r#"const x = "async";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_in_comment_no_match() {
        let violations = check("// async function foo() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_await_in_comment_no_match() {
        let violations = check("/* await fetch('/api') */");
        assert!(violations.is_empty());
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("async function foo() {}");
        assert_eq!(violations[0].rule_name, "async-await");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("async function foo() {}");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
