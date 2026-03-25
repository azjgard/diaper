use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: `.forEach()`, `.map()`, `.filter()`, and `.reduce()` callbacks should
/// not use short abbreviations (4 characters or less) for the current item parameter.
/// For `.reduce()`, the current item is the second parameter; for the others it's the first.
pub struct ShortIterParam;

const SCORE_PER_VIOLATION: u32 = 15;
const MAX_SHORT_LENGTH: usize = 3;

/// The iteration methods we check and which parameter index is the "current item".
const METHODS: &[(&str, usize)] = &[
    ("forEach", 0),
    ("map", 0),
    ("filter", 0),
    ("reduce", 1),
];

impl Rule for ShortIterParam {
    fn name(&self) -> &str {
        "short-iter-param"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/short-iter-param.md"
    }

    fn description(&self) -> &str {
        "Iterator callback item param is 3 chars or less"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["items.map(x => x.id)", "items.forEach(el => el.save())"],
            &["items.map(item => item.id)", "items.forEach(element => element.save())"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("short-iter-param", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &ShortIterParam,
    score: u32,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    let method_name = &source[prop.byte_range()];
                    if let Some(&(_, param_index)) = METHODS.iter().find(|&&(m, _)| m == method_name) {
                        check_callback_param(node, source, violations, rule, score, method_name, param_index);
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

fn check_callback_param(
    call_node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &ShortIterParam,
    score: u32,
    method_name: &str,
    param_index: usize,
) {
    let args = match call_node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    // First argument to the method call is the callback
    let callback = match first_non_paren_child(args) {
        Some(c) => c,
        None => return,
    };

    match callback.kind() {
        "arrow_function" | "function" | "function_expression" => {
            if let Some(param_name) = get_nth_param_name(callback, source, param_index) {
                if param_name == "_" {
                    // Convention for intentionally unused params
                } else if param_name.len() <= MAX_SHORT_LENGTH {
                    let line = source.lines().nth(callback.start_position().row).unwrap_or("");
                    violations.push(RuleViolation {
                        rule_name: rule.name().to_string(),
                        doc_url: rule.doc_url().to_string(),
                        score,
                        code_sample: line.trim().to_string(),
                        fix_suggestion: format!("rename '{param_name}' in .{method_name}() callback to a more descriptive name"),
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

/// Extract the name of the nth parameter from a function/arrow_function node.
fn get_nth_param_name<'a>(func: tree_sitter::Node, source: &'a str, n: usize) -> Option<&'a str> {
    let mut cursor = func.walk();
    for child in func.children(&mut cursor) {
        match child.kind() {
            "identifier" if n == 0 => {
                // Single param arrow function: x => ...
                return Some(&source[child.byte_range()]);
            }
            "formal_parameters" => {
                let mut param_count = 0;
                let mut inner = child.walk();
                for param in child.children(&mut inner) {
                    match param.kind() {
                        "identifier" | "object_pattern" | "array_pattern" => {
                            if param_count == n {
                                return Some(&source[param.byte_range()]);
                            }
                            param_count += 1;
                        }
                        "assignment_pattern" => {
                            if param_count == n {
                                if let Some(left) = param.child_by_field_name("left") {
                                    return Some(&source[left.byte_range()]);
                                }
                            }
                            param_count += 1;
                        }
                        _ => continue,
                    }
                }
                return None;
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
        ShortIterParam.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        ShortIterParam.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- forEach violations ---

    #[test]
    fn test_foreach_short_param() {
        let violations = check("items.forEach((x) => console.log(x));");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 15);
        assert!(violations[0].fix_suggestion.contains("'x'"));
        assert!(violations[0].fix_suggestion.contains(".forEach()"));
    }

    #[test]
    fn test_foreach_short_param_two_chars() {
        let violations = check("items.forEach((el) => console.log(el));");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'el'"));
    }

    #[test]
    fn test_foreach_short_param_three_chars() {
        let violations = check("items.forEach((val) => console.log(val));");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_foreach_ok_four_chars() {
        let violations = check("items.forEach((item) => console.log(item));");
        assert!(violations.is_empty());
    }

    // --- map violations ---

    #[test]
    fn test_map_short_param() {
        let violations = check("const names = users.map((u) => u.name);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'u'"));
        assert!(violations[0].fix_suggestion.contains(".map()"));
    }

    #[test]
    fn test_map_short_param_v() {
        let violations = check("const doubled = arr.map((v) => v * 2);");
        assert_eq!(violations.len(), 1);
    }

    // --- filter violations ---

    #[test]
    fn test_filter_short_param() {
        let violations = check("const active = users.filter((u) => u.active);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains(".filter()"));
    }

    #[test]
    fn test_filter_short_param_el() {
        let violations = check("const big = items.filter((el) => el.size > 10);");
        assert_eq!(violations.len(), 1);
    }

    // --- reduce violations (second param) ---

    #[test]
    fn test_reduce_short_second_param() {
        let violations = check("const total = items.reduce((prevVal, v) => prevVal + v, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'v'"));
        assert!(violations[0].fix_suggestion.contains(".reduce()"));
    }

    #[test]
    fn test_reduce_short_second_param_cur() {
        let violations = check("const total = items.reduce((prevVal, cur) => prevVal + cur, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'cur'"));
    }

    #[test]
    fn test_reduce_long_first_short_second() {
        let violations = check("const total = items.reduce((accumulator, x) => accumulator + x, 0);");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'x'"));
    }

    // --- reduce: does NOT flag first param (that's reduce-param-name's job) ---

    #[test]
    fn test_reduce_short_first_param_not_flagged_here() {
        let violations = check("const total = items.reduce((acc, currentItem) => acc + currentItem, 0);");
        assert!(violations.is_empty());
    }

    // --- No violations ---

    #[test]
    fn test_foreach_descriptive_param() {
        let violations = check("items.forEach((order) => console.log(order));");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_map_descriptive_param() {
        let violations = check("const names = users.map((customer) => customer.name);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_filter_descriptive_param() {
        let violations = check("const active = users.filter((employee) => employee.active);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_descriptive_second_param() {
        let violations = check("const total = items.reduce((prevVal, currentItem) => prevVal + currentItem, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_underscore_param_ignored() {
        let violations = check("items.forEach((_) => doSomething());");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_underscore_param_in_map() {
        let violations = check("items.map((_, i) => i);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_underscore_reduce_second_param_ignored() {
        let violations = check("items.reduce((prevVal, _) => prevVal, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_iteration_calls() {
        let violations = check("const x = arr.sort((a, b) => a - b);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_standalone_function_not_method() {
        let violations = check("map((x) => x * 2);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_variable_callback() {
        let violations = check("items.forEach(processItem);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_args() {
        let violations = check("items.forEach();");
        assert!(violations.is_empty());
    }

    // --- Multiple violations ---

    #[test]
    fn test_multiple_short_params_different_methods() {
        let source = "items.forEach((x) => x);\nitems.map((v) => v);\nitems.filter((e) => e);";
        let violations = check(source);
        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn test_chained_methods() {
        let violations = check("items.filter((x) => x.active).map((v) => v.name);");
        assert_eq!(violations.len(), 2);
    }

    // --- Edge cases ---

    #[test]
    fn test_single_param_arrow_no_parens() {
        let violations = check("items.forEach(x => console.log(x));");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_function_expression() {
        let violations = check("items.forEach(function(el) { console.log(el); });");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiline_callback() {
        let source = r#"items.forEach((el) => {
    console.log(el);
    doSomething(el);
});"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiline_chain_code_sample_shows_callback_line() {
        let source = r#"return (response?.data?.choices?.[0]?.message?.content || "")
    .split('\n')
    .filter((n) => n.trim() !== '');"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        // Code sample should show the line with the callback, not the first line of the chain
        assert!(violations[0].code_sample.contains("filter"));
        assert!(violations[0].code_sample.contains("(n)"));
    }

    #[test]
    fn test_nested_iteration() {
        let source = "items.forEach((el) => el.children.map((c) => c.name));";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_in_comment_not_counted() {
        let violations = check("// items.forEach((x) => x);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_string_not_counted() {
        let violations = check(r#"const s = "items.forEach((x) => x)";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_destructured_param_not_short() {
        // Destructured params are typically descriptive enough by nature
        let violations = check("items.forEach(({ name, id }) => console.log(name, id));");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_single_param_arrow() {
        // Single param arrow in reduce — no second param to check
        let violations = check("items.reduce(prevVal => prevVal);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_exactly_four_chars_ok() {
        let violations = check("items.map((item) => item);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_param_short() {
        let violations = check("items.forEach((el = null) => console.log(el));");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'el'"));
    }

    // --- Excluded paths ---

    #[test]
    fn test_spec_file_excluded() {
        let violations = check_with_path("items.forEach((x) => x);", "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_excluded() {
        let violations = check_with_path("items.forEach((x) => x);", "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("items.forEach((x) => x);");
        assert_eq!(violations[0].rule_name, "short-iter-param");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("items.forEach((x) => x);");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
