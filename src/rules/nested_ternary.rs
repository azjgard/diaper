use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: nested ternary operators add stink.
/// Only flags ternaries nested 2+ levels deep. Single ternaries are fine.
pub struct NestedTernary;

const NESTED_SCORE: u32 = 60;

impl Rule for NestedTernary {
    fn name(&self) -> &str {
        "nested-ternary"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/nested-ternary.md"
    }

    fn description(&self) -> &str {
        "Nested ternary expressions (2+ levels deep)"
    }

    fn default_score(&self) -> u32 {
        NESTED_SCORE
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["const x = a ? b ? c : d : e;"],
            &["function getValue() {\n  if (a) return b;\n  if (c) return d;\n  return e;\n}"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let nested_score = config.rule_score("ternary-nested", NESTED_SCORE);
        let mut violations = Vec::new();
        let mut visited = Vec::new();

        collect_nested_ternaries(tree.root_node(), source, &mut violations, &mut visited, self, nested_score);

        violations
    }
}

fn collect_nested_ternaries(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    visited: &mut Vec<usize>,
    rule: &NestedTernary,
    nested_score: u32,
) {
    if node.kind() == "ternary_expression" && !visited.contains(&node.id()) {
        let depth = count_ternary_depth(node);

        if depth > 1 {
            let line = &source.lines().nth(node.start_position().row).unwrap_or("");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score: nested_score,
                code_sample: line.trim().to_string(),
                fix_suggestion: format!("extract nested ternary ({depth} levels) into a sub-function with early returns for each branch"),
            });
        }

        // Mark all inner ternaries as visited so we don't report them separately
        mark_inner_ternaries(node, visited);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nested_ternaries(child, source, violations, visited, rule, nested_score);
    }
}

/// Count how many levels of ternary nesting exist from this node down.
fn count_ternary_depth(node: tree_sitter::Node) -> u32 {
    if node.kind() != "ternary_expression" {
        return 0;
    }

    let mut max_child_depth = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_depth = find_max_ternary_depth(child);
        if child_depth > max_child_depth {
            max_child_depth = child_depth;
        }
    }

    1 + max_child_depth
}

/// Find the maximum ternary depth in any descendant.
fn find_max_ternary_depth(node: tree_sitter::Node) -> u32 {
    if node.kind() == "ternary_expression" {
        return count_ternary_depth(node);
    }

    let mut max_depth = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let depth = find_max_ternary_depth(child);
        if depth > max_depth {
            max_depth = depth;
        }
    }

    max_depth
}

/// Mark all ternary_expression descendants as visited.
fn mark_inner_ternaries(node: tree_sitter::Node, visited: &mut Vec<usize>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "ternary_expression" {
            visited.push(child.id());
        }
        mark_inner_ternaries(child, visited);
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
        NestedTernary.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Single ternary (no violation) ---

    #[test]
    fn test_simple_ternary_no_violation() {
        let violations = check("const x = a ? b : c;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ternary_in_assignment_no_violation() {
        let violations = check("const result = isReady ? 'yes' : 'no';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ternary_in_return_no_violation() {
        let violations = check("return active ? doThis() : doThat();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_multiline_single_ternary_no_violation() {
        let source = "const x = condition\n  ? valueA\n  : valueB;";
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ternary_with_optional_chaining_no_violation() {
        let violations = check("const x = foo?.bar ? 'yes' : 'no';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_two_separate_ternaries_no_violation() {
        let source = "const a = x ? 1 : 2;\nconst b = y ? 3 : 4;";
        let violations = check(source);
        assert!(violations.is_empty());
    }

    // --- Nested ternary (violation) ---

    #[test]
    fn test_nested_ternary_single_line() {
        let violations = check("const x = a ? b ? c : d : e;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_triple_nested_ternary() {
        let violations = check("const x = a ? b ? c ? d : e : f : g;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
        assert!(violations[0].code_sample.contains("?"));
    }

    #[test]
    fn test_nested_in_else_branch() {
        let violations = check("const x = a ? b : c ? d : e;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_multiline_nested_ternary() {
        let source = r#"  const tern = true
    ? (await fetch("/api"))
      ? console.log("do something totally crazy")
      : false
    : false;"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
        assert!(violations[0].code_sample.contains("true"));
    }

    #[test]
    fn test_mix_single_and_nested() {
        let source = "const a = x ? 1 : 2;\nconst b = x ? y ? 1 : 2 : 3;";
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    // --- No violations ---

    #[test]
    fn test_no_ternary() {
        let violations = check("const x = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_optional_chaining_not_counted() {
        let violations = check("const x = foo?.bar?.baz;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_nullish_coalescing_not_counted() {
        let violations = check("const x = foo ?? bar;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_string_not_counted() {
        let violations = check(r#"const x = "is this a question?";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_single_quote_string() {
        let violations = check("const x = 'what?';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_template_literal() {
        let violations = check("const x = `is this ${y}?`;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ternary_in_comment_not_counted() {
        let violations = check("// const x = a ? b : c;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ternary_in_block_comment_not_counted() {
        let violations = check("/* a ? b : c */");
        assert!(violations.is_empty());
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("const x = a ? b ? c : d : e;");
        assert_eq!(violations[0].rule_name, "nested-ternary");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("const x = a ? b ? c : d : e;");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
