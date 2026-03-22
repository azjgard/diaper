use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: in files inside a folder ending with "-async", all return paths
/// must return a Promise. Early returns like `return ctx` should be
/// `return Promise.resolve(ctx)`. Does not flag returns inside .then() callbacks.
pub struct AsyncPromiseReturn;

const SCORE_PER_VIOLATION: u32 = 15;

impl Rule for AsyncPromiseReturn {
    fn name(&self) -> &str {
        "async-promise-return"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/async-promise-return.md"
    }

    fn description(&self) -> &str {
        "Non-Promise returns in -async folder functions"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["if (!ctx.ready) return ctx;"],
            &["if (!ctx.ready) return Promise.resolve(ctx);"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        // Only applies to files in a folder ending with "-async"
        if !is_in_async_folder(path) {
            return vec![];
        }

        let score = config.rule_score("async-promise-return", SCORE_PER_VIOLATION);

        // Find the default export function
        let func = match find_default_export_function(tree.root_node()) {
            Some(f) => f,
            None => return vec![],
        };

        let mut violations = Vec::new();
        find_non_promise_returns(func, source, &mut violations, self, score, false);
        violations
    }
}

/// Check if the file's immediate parent directory ends with "-async".
fn is_in_async_folder(path: &Path) -> bool {
    path.parent()
        .and_then(|p| p.file_name())
        .is_some_and(|name| name.to_string_lossy().ends_with("-async"))
}

/// Find the function node from a default export.
fn find_default_export_function(root: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "export_statement" {
            continue;
        }
        let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
        if !has_default {
            continue;
        }
        let mut inner = child.walk();
        for c in child.children(&mut inner) {
            match c.kind() {
                "function_declaration" | "function" | "arrow_function" => return Some(c),
                _ => {}
            }
        }
    }
    None
}

/// Recursively find return statements that don't return a Promise.
/// `inside_then` tracks whether we're inside a .then() callback.
fn find_non_promise_returns(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &AsyncPromiseReturn,
    score: u32,
    inside_then: bool,
) {
    // Check if we're entering a .then() callback
    if is_then_callback(node, source) {
        // Don't flag returns inside .then() — they're already in Promise context
        return;
    }

    if node.kind() == "return_statement" && !inside_then {
        if !returns_promise(node, source) {
            let line = source.lines().nth(node.start_position().row).unwrap_or("");
            let return_text = &source[node.byte_range()];
            // Extract what's being returned to suggest wrapping it
            let returned_value = return_text
                .strip_prefix("return ")
                .and_then(|s| s.strip_suffix(';'))
                .unwrap_or("value")
                .trim();

            let is_call = return_value_is_call(node);
            let short_value = if returned_value.len() > 40 {
                format!("{}...", &returned_value[..37])
            } else {
                returned_value.to_string()
            };
            let fix_suggestion = if is_call {
                format!(
                    "either wrap in Promise.resolve(): return Promise.resolve({short_value}) — or if the callee is already async, rename it with an Async suffix"
                )
            } else {
                format!("wrap in Promise.resolve(): return Promise.resolve({short_value})")
            };

            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion,
            });
        }
        // Don't recurse into return statement children
        return;
    }

    // Don't recurse into nested function declarations — they have their own scope
    if node.kind() == "function_declaration"
        || node.kind() == "function"
        || node.kind() == "arrow_function"
    {
        // Only recurse if this is the top-level function we were given
        // (the initial call). For nested functions, stop.
        if node.parent().is_some_and(|p| p.kind() != "export_statement" && p.kind() != "program") {
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_non_promise_returns(child, source, violations, rule, score, inside_then);
    }
}

/// Check if a return statement's value is a function call.
fn return_value_is_call(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_expression" {
            return true;
        }
    }
    false
}

/// Check if a node is a .then() call's callback argument.
/// We want to detect the arrow_function/function inside .then(callback).
fn is_then_callback(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() != "arrow_function" && node.kind() != "function" {
        return false;
    }

    let args = match node.parent().filter(|p| p.kind() == "arguments") {
        Some(a) => a,
        None => return false,
    };
    let call = match args.parent().filter(|p| p.kind() == "call_expression") {
        Some(c) => c,
        None => return false,
    };
    let func = match call.child_by_field_name("function") {
        Some(f) => f,
        None => return false,
    };

    if func.kind() == "member_expression" {
        if let Some(prop) = func.child_by_field_name("property") {
            return &source[prop.byte_range()] == "then";
        }
    }

    false
}

/// Check if a return statement returns a Promise.
fn returns_promise(node: tree_sitter::Node, source: &str) -> bool {
    let text = &source[node.byte_range()];

    // Check common promise patterns
    if text.contains("Promise.resolve")
        || text.contains("Promise.reject")
        || text.contains("Promise.all")
        || text.contains("Promise.race")
        || text.contains("new Promise")
        || text.contains("pipe(")
    {
        return true;
    }

    // Check if it returns a .then() chain (e.g. return SomeModel.update(...).then(...))
    // Look for a call_expression child that's a member_expression ending in .then
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_then_chain(child, source) {
            return true;
        }
        // Check if it returns a call ending in Async() (e.g. someService.fetchDataAsync())
        if returns_async_call(child, source) {
            return true;
        }
    }

    false
}

/// Check if a node is a call expression whose function name ends with "Async".
/// Handles both `fooAsync()` and `some.object.methodAsync()`.
fn returns_async_call(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let func = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return false,
    };
    let name_node = match func.kind() {
        "member_expression" => func.child_by_field_name("property"),
        "identifier" => Some(func),
        _ => None,
    };
    match name_node {
        Some(n) => source[n.byte_range()].ends_with("Async"),
        None => false,
    }
}

/// Recursively check if a node contains a .then() chain.
fn has_then_chain(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    if &source[prop.byte_range()] == "then" {
                        return true;
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_then_chain(child, source) {
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
        AsyncPromiseReturn.check(source, Path::new("src/steps/update-bonus-async/index.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        AsyncPromiseReturn.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_early_return_without_promise() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return ctx;
            return SomeModel.update({}).then(() => ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 15);
        assert!(violations[0].fix_suggestion.contains("Promise.resolve"));
    }

    #[test]
    fn test_multiple_early_returns() {
        let source = r#"export default (ctx) => {
            if (!ctx.a) return ctx;
            if (!ctx.b) return ctx;
            return SomeModel.update({}).then(() => ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_real_world_example() {
        let source = r#"export default (ctx) => {
    const { error, pendingBonusId, tipMatchingBonus } = ctx;

    if (!pendingBonusId) return ctx;

    return PendingBonus.update(
        error
            ? { resolvedAt: new Date(), status: "failed" }
            : { bonusId: tipMatchingBonus.id, resolvedAt: new Date(), status: "processed" },
        { where: { id: pendingBonusId } }
    ).then(() => ctx);
};"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("return ctx"));
    }

    #[test]
    fn test_return_object_without_promise() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return { ...ctx, skipped: true };
            return fetch('/api').then(() => ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_return_non_async_call_flagged() {
        let source = r#"export default (ctx) => {
            return someService.processData(ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_return_non_async_call_suggests_rename_or_wrap() {
        let source = r#"export default (ctx) => {
            return someService.processData(ctx);
        };"#;
        let violations = check(source);
        assert!(violations[0].fix_suggestion.contains("Promise.resolve"));
        assert!(violations[0].fix_suggestion.contains("rename"));
        assert!(violations[0].fix_suggestion.contains("Async"));
    }

    #[test]
    fn test_long_return_value_truncated_in_suggestion() {
        let source = r#"export default (ctx) => {
            return someService.processData({ userId: ctx.userId, orgId: ctx.orgId, extra: ctx.extra });
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        // The fix suggestion should truncate the verbose value
        assert!(violations[0].fix_suggestion.contains("..."));
        assert!(violations[0].fix_suggestion.len() < 200);
    }

    #[test]
    fn test_return_value_suggests_only_wrap() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return ctx;
            return Model.update({}).then(() => ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("Promise.resolve"));
        assert!(!violations[0].fix_suggestion.contains("rename"));
    }

    // --- No violations ---

    #[test]
    fn test_all_returns_promise_resolve() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return Promise.resolve(ctx);
            return SomeModel.update({}).then(() => ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_return_inside_then_not_flagged() {
        let source = r#"export default (ctx) => {
            return SomeModel.update({}).then(() => {
                return ctx;
            });
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_all_then_chains() {
        let source = r#"export default (ctx) => {
            return Model.create({}).then(() => ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_promise_reject() {
        let source = r#"export default (ctx) => {
            if (ctx.error) return Promise.reject(new Error('fail'));
            return fetch('/api').then(() => ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_new_promise() {
        let source = r#"export default (ctx) => {
            return new Promise((resolve) => resolve(ctx));
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_pipe_call_not_flagged() {
        let source = r#"export default (ctx) => {
            return pipe(ctx, [step1, step2]);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_return_async_suffixed_call_not_flagged() {
        let source = r#"export default (ctx) => {
            return someService.processDataAsync(ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_return_async_suffixed_member_call_not_flagged() {
        let source = r#"export default (ctx) => {
            return some.deeply.nested.object.fetchAsync();
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_return_async_suffixed_bare_call_not_flagged() {
        let source = r#"export default (ctx) => {
            return processDataAsync(ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_return_async_suffixed_among_other_returns() {
        // Async call is fine, but early return of ctx is still flagged
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return ctx;
            return someService.processDataAsync(ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("return ctx"));
    }

    #[test]
    fn test_pipe_call_among_other_returns() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return ctx;
            return pipe(ctx, [step1, step2]);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("return ctx"));
    }

    // --- Path filtering ---

    #[test]
    fn test_non_async_folder_skipped() {
        let source = r#"export default (ctx) => {
            if (!ctx.ready) return ctx;
            return fetch('/api').then(() => ctx);
        };"#;
        let violations = check_with_path(source, "src/steps/update-bonus/index.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_folder_detected() {
        let violations = check_with_path(
            r#"export default (ctx) => { if (!ctx.x) return ctx; return fetch('/api').then(() => ctx); };"#,
            "src/pipes/do-thing-async/index.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_nested_async_folder() {
        let violations = check_with_path(
            r#"export default (ctx) => { return ctx; };"#,
            "packages/core/pipes/process-async/index.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_async_ancestor_but_not_parent_skipped() {
        // -async is in the path but not the immediate parent directory
        let violations = check_with_path(
            r#"export default (ctx) => { return ctx; };"#,
            "src/queries/get-data-points-async/build-query/index.js",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_ancestor_with_async_parent() {
        // -async appears twice: ancestor and immediate parent
        let violations = check_with_path(
            r#"export default (ctx) => { return ctx; };"#,
            "src/queries/get-data-points-async/build-query-async/index.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_child_of_async_folder_not_flagged() {
        // File is in a subfolder of -async, not directly in -async
        let violations = check_with_path(
            r#"export default (ctx) => { if (!ctx.x) return ctx; return fetch('/api').then(() => ctx); };"#,
            "src/pipes/do-thing-async/steps/myStep.js",
        );
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_default_export() {
        let violations = check("function foo() { return 42; }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_return_statements() {
        let violations = check("export default (ctx) => { console.log(ctx); };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_function_declaration_style() {
        let source = r#"export default function processAsync(ctx) {
            if (!ctx.ready) return ctx;
            return Model.update({}).then(() => ctx);
        }"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(r#"export default (ctx) => { return ctx; };"#);
        assert_eq!(violations[0].rule_name, "async-promise-return");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(r#"export default (ctx) => { return ctx; };"#);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
