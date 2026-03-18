use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: functions that are exported as named (non-default) exports add 50 stink each.
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

    fn check(&self, source: &str, _path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let score = config.rule_score("non-default-export", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_named_exports(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

fn collect_named_exports(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &NonDefaultExport,
    score: u32,
) {
    if node.kind() == "export_statement" {
        let has_default = node.children(&mut node.walk())
            .any(|c| c.kind() == "default");

        if !has_default && contains_function(node) {
            let fn_name = extract_exported_name(node, source).unwrap_or("anonymous");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: format!("export {{ {fn_name} }}"),
                fix_suggestion: format!("use export default for {fn_name} or move it to its own file"),
            });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_named_exports(child, source, violations, rule, score);
    }
}

/// Check if a node or its children contain a function declaration,
/// arrow function, or function expression.
fn contains_function(node: tree_sitter::Node) -> bool {
    match node.kind() {
        "function_declaration" | "arrow_function" | "function" | "generator_function_declaration" => {
            return true;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_function(child) {
            return true;
        }
    }

    false
}

/// Extract the name of the exported function.
fn extract_exported_name<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    return Some(&source[name_node.byte_range()]);
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                // export const foo = () => {}
                let mut inner = child.walk();
                for decl in child.children(&mut inner) {
                    if decl.kind() == "variable_declarator" {
                        if let Some(name_node) = decl.child_by_field_name("name") {
                            return Some(&source[name_node.byte_range()]);
                        }
                    }
                }
            }
            _ => {}
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
        NonDefaultExport.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations ---

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
    fn test_no_export() {
        let violations = check("function foo() {}");
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
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_exports() {
        let violations = check("const x = 1;\nfunction foo() {}\nconsole.log(foo());");
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
