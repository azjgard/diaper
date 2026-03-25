use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: `.reduce()` callback's first parameter must be named `prevVal`.
/// 70-point violation if not.
pub struct ReduceParamName;

const SCORE_PER_VIOLATION: u32 = 70;

impl Rule for ReduceParamName {
    fn name(&self) -> &str {
        "reduce-param-name"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/reduce-param-name.md"
    }

    fn description(&self) -> &str {
        ".reduce() callback first param must be named prevVal"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["arr.reduce((acc, item) => acc + item, 0)"],
            &["arr.reduce((prevVal, item) => prevVal + item, 0)"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("reduce-param-name", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_reduce_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

fn collect_reduce_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &ReduceParamName,
    score: u32,
) {
    // Look for call_expression nodes where the function is a member_expression ending in .reduce
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    if &source[prop.byte_range()] == "reduce" {
                        check_reduce_callback(node, source, violations, rule, score);
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_reduce_violations(child, source, violations, rule, score);
    }
}

/// Check the first argument of a .reduce() call to see if it's a callback
/// whose first parameter is named "prevVal".
fn check_reduce_callback(
    call_node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &ReduceParamName,
    score: u32,
) {
    let args = match call_node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    // First argument to .reduce() is the callback
    let callback = match first_non_paren_child(args) {
        Some(c) => c,
        None => return,
    };

    match callback.kind() {
        "arrow_function" | "function" | "function_expression" => {
            if let Some(first_param_name) = get_first_param_name(callback, source) {
                if first_param_name != "prevVal" {
                    let line = source.lines().nth(call_node.start_position().row).unwrap_or("");
                    violations.push(RuleViolation {
                        rule_name: rule.name().to_string(),
                        doc_url: rule.doc_url().to_string(),
                        score,
                        code_sample: line.trim().to_string(),
                        fix_suggestion: format!("rename .reduce() callback first parameter from '{first_param_name}' to 'prevVal'"),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Get the first non-punctuation child of an arguments node.
fn first_non_paren_child(args: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
            return Some(child);
        }
    }
    None
}

/// Extract the name of the first parameter from a function/arrow_function node.
fn get_first_param_name<'a>(func: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    // Arrow functions can have a single identifier parameter (no parens)
    // or a formal_parameters node
    let mut cursor = func.walk();
    for child in func.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                // Single param arrow function: x => ...
                return Some(&source[child.byte_range()]);
            }
            "formal_parameters" => {
                // (x, y) => ... or function(x, y) { ... }
                let mut inner = child.walk();
                for param in child.children(&mut inner) {
                    match param.kind() {
                        "identifier" => return Some(&source[param.byte_range()]),
                        // Destructured parameter like { a, b }
                        "object_pattern" | "array_pattern" => return Some(&source[param.byte_range()]),
                        // Assignment pattern like x = default
                        "assignment_pattern" => {
                            if let Some(left) = param.child_by_field_name("left") {
                                return Some(&source[left.byte_range()]);
                            }
                        }
                        _ => continue,
                    }
                }
            }
            _ => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        ReduceParamName.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        ReduceParamName.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_arrow_function_wrong_name() {
        let violations = check("const total = items.reduce((acc, item) => acc + item, 0);");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 70);
        assert!(violations[0].fix_suggestion.contains("'acc'"));
        assert!(violations[0].fix_suggestion.contains("'prevVal'"));
    }

    #[test]
    fn test_function_expression_wrong_name() {
        let violations = check("const total = items.reduce(function(sum, item) { return sum + item; }, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'sum'"));
    }

    #[test]
    fn test_accumulator_name() {
        let violations = check("const result = arr.reduce((accumulator, val) => accumulator + val, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'accumulator'"));
    }

    #[test]
    fn test_result_name() {
        let violations = check("const x = arr.reduce((result, item) => ({ ...result, [item.id]: item }), {});");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_single_param_arrow_wrong_name() {
        // Single param with no parens — unusual for reduce but syntactically valid
        let violations = check("const x = arr.reduce(acc => acc, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'acc'"));
    }

    #[test]
    fn test_chained_reduce() {
        let violations = check("const x = arr.filter(Boolean).reduce((acc, v) => acc + v, 0);");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_reduces() {
        let source = "const a = x.reduce((acc, v) => acc + v, 0);\nconst b = y.reduce((sum, v) => sum + v, 0);";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    // --- No violations ---

    #[test]
    fn test_correct_name_prevval() {
        let violations = check("const total = items.reduce((prevVal, item) => prevVal + item, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_correct_name_function_expression() {
        let violations = check("const total = items.reduce(function(prevVal, item) { return prevVal + item; }, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_correct_name_single_param() {
        let violations = check("const x = arr.reduce(prevVal => prevVal, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_reduce_calls() {
        let violations = check("const x = arr.map(item => item * 2);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_not_method() {
        // reduce as a standalone function call, not a method
        let violations = check("const x = reduce((acc, v) => acc + v, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_with_no_args() {
        let violations = check("const x = arr.reduce();");
        assert!(violations.is_empty());
    }

    // --- Excluded paths ---

    #[test]
    fn test_spec_file_excluded() {
        let violations = check_with_path("const x = arr.reduce((acc, v) => acc + v, 0);", "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_excluded() {
        let violations = check_with_path("const x = arr.reduce((acc, v) => acc + v, 0);", "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_reduce_with_variable_callback() {
        // Passing a variable reference instead of inline function — no violation
        let violations = check("const x = arr.reduce(myReducer, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_multiline() {
        let source = r#"const result = items.reduce((acc, item) => {
    acc[item.id] = item;
    return acc;
}, {});"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_nested_reduce() {
        let source = "const x = arr.reduce((acc, group) => acc.concat(group.reduce((innerAcc, v) => innerAcc + v, 0)), []);";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_reduce_in_comment_not_counted() {
        let violations = check("// arr.reduce((acc, v) => acc + v, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_in_string_not_counted() {
        let violations = check(r#"const x = "arr.reduce((acc, v) => acc + v, 0)";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_param_wrong_name() {
        let violations = check("const x = arr.reduce((acc = {}, v) => ({ ...acc, ...v }), {});");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'acc'"));
    }

    #[test]
    fn test_default_param_correct_name() {
        let violations = check("const x = arr.reduce((prevVal = {}, v) => ({ ...prevVal, ...v }), {});");
        assert!(violations.is_empty());
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("const x = arr.reduce((acc, v) => acc + v, 0);");
        assert_eq!(violations[0].rule_name, "reduce-param-name");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("const x = arr.reduce((acc, v) => acc + v, 0);");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
