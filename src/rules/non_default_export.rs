use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: any function defined in a file that is not the default export adds 50 stink.
/// This includes named exports AND local (non-exported) functions.
/// Encourages single-responsibility files with one default export.
pub struct NonDefaultExport;

const SCORE_PER_VIOLATION: u32 = 50;

impl Rule for NonDefaultExport {
    fn name(&self) -> &str {
        "non-default-export"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/non-default-export.md"
    }

    fn description(&self) -> &str {
        "Functions that aren't the default export"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["export function helper() {}"],
            &["export default function main() {}"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("non-default-export", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        let root = tree.root_node();

        // Find the default export's function node ID so we can skip it
        let default_fn_id = find_default_export_function_id(root);

        collect_non_default_functions(root, source, &mut violations, self, score, default_fn_id);
        violations
    }
}

/// Find the node ID of the function inside a default export statement.
fn find_default_export_function_id(root: tree_sitter::Node) -> Option<usize> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "export_statement" {
            continue;
        }
        let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
        if !has_default {
            continue;
        }
        // Find the function inside the default export
        let mut inner = child.walk();
        for c in child.children(&mut inner) {
            match c.kind() {
                "function_declaration" | "function" | "arrow_function" | "generator_function_declaration" => {
                    return Some(c.id());
                }
                _ => {}
            }
        }
        // The export_statement itself covers the default export
        return Some(child.id());
    }
    None
}

/// Walk top-level statements and find functions that aren't the default export.
fn collect_non_default_functions(
    root: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &NonDefaultExport,
    score: u32,
    default_fn_id: Option<usize>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        // Skip the default export entirely
        if let Some(id) = default_fn_id {
            if child.id() == id {
                continue;
            }
        }

        match child.kind() {
            // Top-level function declaration: function foo() {}
            "function_declaration" | "generator_function_declaration" => {
                let fn_name = child.child_by_field_name("name")
                    .map(|n| &source[n.byte_range()])
                    .unwrap_or("anonymous");
                if is_recursive_function(child, source, fn_name) {
                    continue;
                }
                violations.push(RuleViolation {
                    rule_name: rule.name().to_string(),
                    doc_url: rule.doc_url().to_string(),
                    score,
                    code_sample: format!("function {fn_name}"),
                    fix_suggestion: format!("move {fn_name} to a nested module and import it instead"),
                });
            }
            // Named export with function: export function foo() {}
            "export_statement" => {
                let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
                if has_default {
                    continue;
                }
                if let Some((fn_name, fn_node)) = extract_function_from_export(child, source) {
                    if is_recursive_function(fn_node, source, fn_name) {
                        continue;
                    }
                    violations.push(RuleViolation {
                        rule_name: rule.name().to_string(),
                        doc_url: rule.doc_url().to_string(),
                        score,
                        code_sample: format!("export {{ {fn_name} }}"),
                        fix_suggestion: format!("move {fn_name} to a nested module and import it instead"),
                    });
                }
            }
            // Variable declaration with function: const foo = () => {}
            "lexical_declaration" | "variable_declaration" => {
                if let Some((fn_name, fn_node)) = extract_function_from_var_decl(child, source) {
                    if is_recursive_function(fn_node, source, fn_name) {
                        continue;
                    }
                    violations.push(RuleViolation {
                        rule_name: rule.name().to_string(),
                        doc_url: rule.doc_url().to_string(),
                        score,
                        code_sample: format!("const {fn_name} = ..."),
                        fix_suggestion: format!("move {fn_name} to a nested module and import it instead"),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Extract function name and function node from a named export statement, if it contains a function.
fn extract_function_from_export<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> Option<(&'a str, tree_sitter::Node<'a>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                let name = child.child_by_field_name("name")
                    .map(|n| &source[n.byte_range()])?;
                return Some((name, child));
            }
            "lexical_declaration" | "variable_declaration" => {
                return extract_function_from_var_decl(child, source);
            }
            _ => {}
        }
    }
    None
}

/// If a variable declaration assigns a function/arrow, return the variable name and the function node.
fn extract_function_from_var_decl<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> Option<(&'a str, tree_sitter::Node<'a>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child.child_by_field_name("name")?;
            let value = child.child_by_field_name("value")?;
            match value.kind() {
                "arrow_function" | "function" => {
                    return Some((&source[name.byte_range()], value));
                }
                // Check for async arrow: the value might be the arrow_function itself
                // or it could be wrapped
                _ => {
                    if contains_function_node(value) {
                        return Some((&source[name.byte_range()], value));
                    }
                }
            }
        }
    }
    None
}

/// Check if a node is or contains a function expression.
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

/// Check if a function is recursive (calls itself by name in its body).
fn is_recursive_function(node: tree_sitter::Node, source: &str, fn_name: &str) -> bool {
    let body = match node.child_by_field_name("body") {
        Some(b) => b,
        None => return false,
    };
    body_calls_name(body, source, fn_name)
}

/// Recursively check if any call_expression in the subtree calls the given name.
fn body_calls_name(node: tree_sitter::Node, source: &str, fn_name: &str) -> bool {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "identifier" && &source[func.byte_range()] == fn_name {
                return true;
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if body_calls_name(child, source, fn_name) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        NonDefaultExport.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations: named exports ---

    #[test]
    fn test_export_function() {
        let violations = check("export function foo() {}");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
    }

    #[test]
    fn test_export_async_function() {
        let violations = check("export async function fetchData() {}");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_arrow() {
        let violations = check("export const foo = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_arrow_with_params() {
        let violations = check("export const foo = (a, b) => a + b;");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_function_expression() {
        let violations = check("export const foo = function() {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_async_arrow() {
        let violations = check("export const foo = async () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_let_arrow() {
        let violations = check("export let handler = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_var_arrow() {
        let violations = check("export var handler = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_exports() {
        let source = "export function foo() {}\nexport function bar() {}\nexport const baz = () => {};";
        let violations = check(source);
        assert_eq!(violations.len(), 3);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 150);
    }

    // --- Violations: local (non-exported) functions ---

    #[test]
    fn test_local_function_declaration() {
        let violations = check("function helper() {}");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("helper"));
    }

    #[test]
    fn test_local_const_arrow() {
        let violations = check("const helper = () => {};");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("helper"));
    }

    #[test]
    fn test_local_const_function_expression() {
        let violations = check("const helper = function() {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_local_function_alongside_default_export() {
        let source = "function helper() {}\nexport default function main() {}";
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("helper"));
    }

    #[test]
    fn test_local_arrow_alongside_default_export() {
        let source = "const helper = () => {};\nexport default () => {};";
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("helper"));
    }

    #[test]
    fn test_multiple_local_functions() {
        let source = "function a() {}\nfunction b() {}\nconst c = () => {};";
        let violations = check(source);
        assert_eq!(violations.len(), 3);
    }

    // --- OK: recursive functions ---

    #[test]
    fn test_recursive_function_declaration() {
        let violations = check("function factorial(n) { return n <= 1 ? 1 : n * factorial(n - 1); }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_recursive_function_alongside_default_export() {
        let source = "function traverse(node) { node.children.forEach(c => traverse(c)); }\nexport default function main() {}";
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_recursive_const_function() {
        let violations = check("const fib = function fib(n) { return n <= 1 ? n : fib(n - 1) + fib(n - 2); };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_recursive_exported_function() {
        let violations = check("export function walk(node) { node.children.forEach(c => walk(c)); }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_recursive_arrow_function() {
        let violations = check("const countdown = (n) => { if (n > 0) countdown(n - 1); };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_recursive_still_flagged() {
        let violations = check("function helper() { return 42; }");
        assert_eq!(violations.len(), 1);
    }

    // --- OK (no violations) ---

    #[test]
    fn test_default_export_function() {
        let violations = check("export default function foo() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_export_anonymous() {
        let violations = check("export default function() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_export_expression() {
        let violations = check("export default () => {};");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_non_function() {
        let violations = check("export const FOO = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_string() {
        let violations = check("export const NAME = 'hello';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_object() {
        let violations = check("export const config = { port: 3000 };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_const_non_function() {
        let violations = check("const x = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_only_constants() {
        let violations = check("const x = 1;\nconst y = 'hello';\nconsole.log(x, y);");
        assert!(violations.is_empty());
    }

    // --- Function name extraction ---

    #[test]
    fn test_message_contains_function_name() {
        let violations = check("export function myHelper() {}");
        assert!(violations[0].code_sample.contains("myHelper"));
    }

    #[test]
    fn test_message_contains_const_name() {
        let violations = check("export const doStuff = () => {};");
        assert!(violations[0].code_sample.contains("doStuff"));
    }

    #[test]
    fn test_message_contains_async_function_name() {
        let violations = check("export async function loadData() {}");
        assert!(violations[0].code_sample.contains("loadData"));
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("export function foo() {}");
        assert_eq!(violations[0].rule_name, "non-default-export");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("export function foo() {}");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
