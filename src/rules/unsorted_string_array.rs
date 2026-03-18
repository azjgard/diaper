use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: arrays of strings that are not in alphabetical order add 5 stink each.
pub struct UnsortedStringArray;

const SCORE_PER_VIOLATION: u32 = 5;

impl Rule for UnsortedStringArray {
    fn name(&self) -> &str {
        "unsorted-string-array"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/unsorted-string-array.md"
    }

    fn check(&self, source: &str, _path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let score = config.rule_score("unsorted-string-array", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        find_unsorted_arrays(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Walk the AST looking for array literals containing only strings that aren't sorted.
fn find_unsorted_arrays(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &UnsortedStringArray,
    score: u32,
) {
    if node.kind() == "array" {
        let strings = extract_string_elements(node, source);
        // Only flag arrays with 2+ string elements and no non-string elements
        if strings.len() >= 2 && is_all_strings(node) {
            if !is_sorted(&strings) {
                let line = source.lines().nth(node.start_position().row).unwrap_or("");
                violations.push(RuleViolation {
                    rule_name: rule.name().to_string(),
                    doc_url: rule.doc_url().to_string(),
                    score,
                    code_sample: line.trim().to_string(),
                    fix_suggestion: "alphabetize this string array — but ONLY if the order does not affect functionality (e.g. execution order, priority, or precedence)".to_string(),
                });
                // Don't recurse into this array's children
                return;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_unsorted_arrays(child, source, violations, rule, score);
    }
}

/// Check if all named children of the array are string nodes.
fn is_all_strings(array: tree_sitter::Node) -> bool {
    let mut count = 0;
    let mut cursor = array.walk();
    for child in array.children(&mut cursor) {
        if child.is_named() && child.kind() != "comment" {
            if child.kind() != "string" {
                return false;
            }
            count += 1;
        }
    }
    count >= 2
}

/// Extract string values from an array node.
fn extract_string_elements<'a>(array: tree_sitter::Node, source: &'a str) -> Vec<&'a str> {
    let mut strings = Vec::new();
    let mut cursor = array.walk();
    for child in array.children(&mut cursor) {
        if child.kind() == "string" {
            if let Some(content) = extract_string_content(child, source) {
                strings.push(content);
            }
        }
    }
    strings
}

/// Extract the content of a string node (without quotes).
fn extract_string_content<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_fragment" {
            return Some(&source[child.byte_range()]);
        }
    }
    None
}

/// Check if a slice of strings is in alphabetical (case-insensitive) order.
fn is_sorted(strings: &[&str]) -> bool {
    strings.windows(2).all(|w| w[0].to_lowercase() <= w[1].to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        UnsortedStringArray.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_unsorted_array() {
        let violations = check(r#"const x = ["banana", "apple"];"#);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 5);
    }

    #[test]
    fn test_unsorted_single_quotes() {
        let violations = check("const x = ['zebra', 'aardvark'];");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_unsorted_three_elements() {
        let violations = check(r#"const x = ["cherry", "apple", "banana"];"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_unsorted_case_insensitive() {
        let violations = check(r#"const x = ["Banana", "apple"];"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_unsorted_arrays() {
        let source = r#"
const a = ["z", "a"];
const b = ["y", "b"];
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    // --- No violations ---

    #[test]
    fn test_sorted_array() {
        let violations = check(r#"const x = ["apple", "banana", "cherry"];"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_sorted_case_insensitive() {
        let violations = check(r#"const x = ["Apple", "banana", "Cherry"];"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_single_element() {
        let violations = check(r#"const x = ["only"];"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_array() {
        let violations = check("const x = [];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mixed_types_not_flagged() {
        // Array with non-string elements should not be flagged
        let violations = check(r#"const x = ["b", "a", 42];"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_number_array_not_flagged() {
        let violations = check("const x = [3, 1, 2];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_object_array_not_flagged() {
        let violations = check("const x = [{ a: 1 }, { b: 2 }];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_variable_array_not_flagged() {
        let violations = check("const x = [b, a];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_equal_strings_ok() {
        let violations = check(r#"const x = ["same", "same"];"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_nested_sorted_array_in_unsorted_outer() {
        // Inner array is unsorted, outer is not a string array
        let violations = check(r#"const x = [["z", "a"], ["b", "c"]];"#);
        assert_eq!(violations.len(), 1); // only the inner ["z", "a"]
    }

    #[test]
    fn test_array_in_function_call() {
        let violations = check(r#"doSomething(["z", "a"]);"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiline_unsorted_array() {
        let source = r#"const x = [
  "zebra",
  "apple",
  "banana"
];"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_array_in_comment_no_match() {
        let violations = check(r#"// const x = ["z", "a"];"#);
        assert!(violations.is_empty());
    }

    // --- Fix suggestion ---

    #[test]
    fn test_fix_suggestion_warns_about_order_dependency() {
        let violations = check(r#"const x = ["banana", "apple"];"#);
        assert!(violations[0].fix_suggestion.contains("ONLY if"));
        assert!(violations[0].fix_suggestion.contains("functionality"));
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(r#"const x = ["b", "a"];"#);
        assert_eq!(violations[0].rule_name, "unsorted-string-array");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(r#"const x = ["b", "a"];"#);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
