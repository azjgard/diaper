use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: use a `distinct()` utility instead of manual dedup patterns.
/// Flags: `[...new Set(x)]`, `Array.from(new Set(x))`, and `.reduce()` with `indexOf`/`push`.
pub struct DistinctArray;

const SCORE_PER_VIOLATION: u32 = 20;

impl Rule for DistinctArray {
    fn name(&self) -> &str {
        "distinct-array"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/distinct-array.md"
    }

    fn description(&self) -> &str {
        "Manual array dedup instead of distinct()"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["[...new Set(arr)]", "Array.from(new Set(arr))"],
            &["distinct(arr)"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("distinct-array", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &DistinctArray,
    score: u32,
) {
    // Pattern 1: [...new Set(x)]
    // AST: array > spread_element > new_expression where constructor is "Set"
    if node.kind() == "array" {
        if is_spread_new_set(node, source) {
            let line = source.lines().nth(node.start_position().row).unwrap_or("");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion: "use distinct() utility instead of [...new Set()]".to_string(),
            });
            // Don't recurse into this node's children
            return;
        }
    }

    // Pattern 2: Array.from(new Set(x))
    // AST: call_expression where function is member_expression "Array.from"
    //      and first argument is new_expression with constructor "Set"
    if node.kind() == "call_expression" {
        if is_array_from_new_set(node, source) {
            let line = source.lines().nth(node.start_position().row).unwrap_or("");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion: "use distinct() utility instead of Array.from(new Set())".to_string(),
            });
            return;
        }

        // Pattern 3: .reduce() with indexOf + push (manual dedup)
        if is_reduce_dedup(node, source) {
            let line = source.lines().nth(node.start_position().row).unwrap_or("");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion: "use distinct() utility instead of manual dedup with reduce".to_string(),
            });
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_violations(child, source, violations, rule, score);
    }
}

/// Check if an array node is `[...new Set(x)]`.
fn is_spread_new_set(array_node: tree_sitter::Node, source: &str) -> bool {
    let mut cursor = array_node.walk();
    for child in array_node.children(&mut cursor) {
        if child.kind() == "spread_element" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "new_expression" && is_set_constructor(inner, source) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a call_expression is `Array.from(new Set(x))`.
fn is_array_from_new_set(call_node: tree_sitter::Node, source: &str) -> bool {
    let func = match call_node.child_by_field_name("function") {
        Some(f) => f,
        None => return false,
    };

    if func.kind() != "member_expression" {
        return false;
    }

    let obj = match func.child_by_field_name("object") {
        Some(o) => o,
        None => return false,
    };
    let prop = match func.child_by_field_name("property") {
        Some(p) => p,
        None => return false,
    };

    if &source[obj.byte_range()] != "Array" || &source[prop.byte_range()] != "from" {
        return false;
    }

    let args = match call_node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return false,
    };

    // Check if first argument is `new Set(...)`
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() == "new_expression" && is_set_constructor(child, source) {
            return true;
        }
    }

    false
}

/// Check if a new_expression has "Set" as its constructor.
fn is_set_constructor(new_expr: tree_sitter::Node, source: &str) -> bool {
    let constructor = match new_expr.child_by_field_name("constructor") {
        Some(c) => c,
        None => return false,
    };
    &source[constructor.byte_range()] == "Set"
}

/// Check if a call_expression is `.reduce((acc, item) => { ... indexOf ... push ... }, [])`.
fn is_reduce_dedup(call_node: tree_sitter::Node, source: &str) -> bool {
    let func = match call_node.child_by_field_name("function") {
        Some(f) => f,
        None => return false,
    };

    if func.kind() != "member_expression" {
        return false;
    }

    let prop = match func.child_by_field_name("property") {
        Some(p) => p,
        None => return false,
    };

    if &source[prop.byte_range()] != "reduce" {
        return false;
    }

    // Check if the reduce body contains both indexOf and push — strong signal of manual dedup
    let call_text = &source[call_node.byte_range()];
    call_text.contains("indexOf") && call_text.contains("push")
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        DistinctArray.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations: spread new Set ---

    #[test]
    fn test_spread_new_set() {
        let violations = check("const unique = [...new Set(someList)];");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_spread_new_set_no_args() {
        let violations = check("const unique = [...new Set()];");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_spread_new_set_inline() {
        let violations = check("doSomething([...new Set(items)]);");
        assert_eq!(violations.len(), 1);
    }

    // --- Violations: Array.from(new Set) ---

    #[test]
    fn test_array_from_new_set() {
        let violations = check("const unique = Array.from(new Set(someList));");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_array_from_new_set_no_args() {
        let violations = check("const unique = Array.from(new Set());");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_array_from_new_set_inline() {
        let violations = check("return Array.from(new Set(list));");
        assert_eq!(violations.len(), 1);
    }

    // --- Violations: reduce dedup ---

    #[test]
    fn test_reduce_dedup() {
        let source = r#"const deduped = arr.reduce((acc, item) => {
  if (acc.indexOf(item) === -1) acc.push(item);
  return acc;
}, []);"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_reduce_dedup_one_line() {
        let source = "const d = arr.reduce((a, i) => { if (a.indexOf(i) === -1) a.push(i); return a; }, []);";
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- Violations: multiple in one file ---

    #[test]
    fn test_multiple_patterns() {
        let source = "const a = [...new Set(x)];\nconst b = Array.from(new Set(y));";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 40);
    }

    // --- No violations ---

    #[test]
    fn test_distinct_call_ok() {
        let violations = check("const unique = distinct(someArray);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_new_set_without_spread() {
        let violations = check("const s = new Set(items);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_array_from_without_set() {
        let violations = check("const a = Array.from(document.querySelectorAll('div'));");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_spread_without_set() {
        let violations = check("const a = [...items];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_without_dedup() {
        let violations = check("const sum = arr.reduce((acc, n) => acc + n, 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_with_only_push() {
        let violations = check("const flat = arr.reduce((acc, item) => { acc.push(item); return acc; }, []);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_reduce_with_only_indexof() {
        let violations = check("const idx = arr.reduce((acc, item) => acc + arr.indexOf(item), 0);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_indexof_push_not_in_reduce() {
        let violations = check("if (arr.indexOf(x) === -1) arr.push(x);");
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_comment() {
        let violations = check("// const a = [...new Set(x)];");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_block_comment() {
        let violations = check("/* Array.from(new Set(x)) */");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_string() {
        let violations = check(r#"const s = "const a = [...new Set(x)]";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_in_template_literal() {
        let violations = check("const s = `${[...new Set(x)]}`;");
        // Template literals do get parsed as real code by tree-sitter,
        // so the spread+Set inside the expression will be detected.
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_spec_file_excluded() {
        let tree = parse_js("const a = [...new Set(x)];").unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        let violations = DistinctArray.check(
            "const a = [...new Set(x)];",
            Path::new("src/index.spec.js"),
            &tree,
            &mut cache,
            &config,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migration_excluded() {
        let tree = parse_js("const a = [...new Set(x)];").unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        let violations = DistinctArray.check(
            "const a = [...new Set(x)];",
            Path::new("db/migrations/001_init.js"),
            &tree,
            &mut cache,
            &config,
        );
        assert!(violations.is_empty());
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("const a = [...new Set(x)];");
        assert_eq!(violations[0].rule_name, "distinct-array");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("const a = [...new Set(x)];");
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_fix_suggestion_mentions_distinct() {
        let v1 = check("const a = [...new Set(x)];");
        assert!(v1[0].fix_suggestion.contains("distinct()"));

        let v2 = check("const a = Array.from(new Set(x));");
        assert!(v2[0].fix_suggestion.contains("distinct()"));
    }
}
