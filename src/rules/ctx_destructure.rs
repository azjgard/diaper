use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: in pipe flow functions (default export with `ctx` param),
/// accessing ctx fields directly (ctx.foo) instead of destructuring
/// adds 10 stink per access. Encourages `const { foo } = ctx;`
pub struct CtxDestructure;

const SCORE_PER_VIOLATION: u32 = 10;

impl Rule for CtxDestructure {
    fn name(&self) -> &str {
        "ctx-destructure"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/ctx-destructure.md"
    }

    fn check(&self, source: &str, _path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let score = config.rule_score("ctx-destructure", SCORE_PER_VIOLATION);

        // Only applies to files with a default export function that has a single `ctx` param
        let func = match find_default_export_with_ctx(tree.root_node(), source) {
            Some(f) => f,
            None => return vec![],
        };

        let mut violations = Vec::new();
        find_ctx_member_access(func, source, &mut violations, self, score);
        violations
    }
}

/// Find the function body of a default export with a single `ctx` parameter.
/// Returns the function node if found.
fn find_default_export_with_ctx<'a>(root: tree_sitter::Node<'a>, source: &str) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "export_statement" {
            continue;
        }

        let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
        if !has_default {
            continue;
        }

        // Find the function inside the export
        let mut inner = child.walk();
        for c in child.children(&mut inner) {
            match c.kind() {
                "function_declaration" | "function" | "arrow_function" => {
                    if has_single_ctx_param(c, source) {
                        return Some(c);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Check if a function node has exactly one parameter named "ctx".
fn has_single_ctx_param(node: tree_sitter::Node, source: &str) -> bool {
    let params = match node.child_by_field_name("parameters") {
        Some(p) => p,
        None => {
            // Arrow functions can have a single param without parens: ctx => { ... }
            // In that case the parameter is the first named child of kind "identifier"
            if node.kind() == "arrow_function" {
                if let Some(param) = node.child_by_field_name("parameter") {
                    return &source[param.byte_range()] == "ctx";
                }
            }
            return false;
        }
    };

    if params.named_child_count() != 1 {
        return false;
    }

    match params.named_child(0) {
        Some(param) => &source[param.byte_range()] == "ctx",
        None => false,
    }
}

/// Recursively find `ctx.something` member access expressions.
fn find_ctx_member_access(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &CtxDestructure,
    score: u32,
) {
    if node.kind() == "member_expression" {
        if let Some(object) = node.child_by_field_name("object") {
            if object.kind() == "identifier" && &source[object.byte_range()] == "ctx" {
                if let Some(property) = node.child_by_field_name("property") {
                    let prop_name = &source[property.byte_range()];
                    let line = source.lines().nth(node.start_position().row).unwrap_or("");
                    violations.push(RuleViolation {
                        rule_name: rule.name().to_string(),
                        doc_url: rule.doc_url().to_string(),
                        score,
                        code_sample: line.trim().to_string(),
                        fix_suggestion: format!("destructure ctx: const {{ {prop_name} }} = ctx"),
                    });
                }
                // Don't recurse into this node's children — we already found the access
                return;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_ctx_member_access(child, source, violations, rule, score);
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
        CtxDestructure.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_ctx_dot_access() {
        let violations = check("export default (ctx) => { return ctx.foo; };");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_multiple_ctx_accesses() {
        let source = r#"export default (ctx) => {
            const a = ctx.foo;
            const b = ctx.bar;
            return { ...ctx, baz: a + b };
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 20);
    }

    #[test]
    fn test_ctx_access_in_function_declaration() {
        let violations = check("export default function myStep(ctx) { return ctx.foo; }");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_ctx_access_nested() {
        let source = r#"export default (ctx) => {
            if (ctx.enabled) {
                console.log(ctx.name);
            }
            return { ...ctx };
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_ctx_access_in_spread_ok() {
        // ...ctx is a spread, not member access — should not trigger
        let violations = check("export default (ctx) => ({ ...ctx });");
        assert!(violations.is_empty());
    }

    // --- No violations ---

    #[test]
    fn test_destructured_access_ok() {
        let source = r#"export default (ctx) => {
            const { foo, bar } = ctx;
            return { ...ctx, baz: foo + bar };
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_default_export() {
        let violations = check("export function foo(ctx) { return ctx.bar; }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_ctx_param() {
        let violations = check("export default (data) => { return data.foo; };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_multiple_params() {
        let violations = check("export default (ctx, extra) => { return ctx.foo; };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_export() {
        let violations = check("const fn = (ctx) => ctx.foo;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_other_object_member_access_ok() {
        // Accessing other objects is fine
        let source = r#"export default (ctx) => {
            const { foo } = ctx;
            return { ...ctx, bar: foo.length };
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ctx_in_comment_no_match() {
        let violations = check("export default (ctx) => { /* ctx.foo */ return { ...ctx }; };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ctx_in_string_no_match() {
        let violations = check(r#"export default (ctx) => { const s = "ctx.foo"; return { ...ctx }; };"#);
        assert!(violations.is_empty());
    }

    // --- Fix suggestion ---

    #[test]
    fn test_fix_suggestion_contains_property() {
        let violations = check("export default (ctx) => { return ctx.myProp; };");
        assert!(violations[0].fix_suggestion.contains("myProp"));
        assert!(violations[0].fix_suggestion.contains("destructure"));
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("export default (ctx) => { return ctx.foo; };");
        assert_eq!(violations[0].rule_name, "ctx-destructure");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("export default (ctx) => { return ctx.foo; };");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
