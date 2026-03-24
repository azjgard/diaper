use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: files that return a Promise (via .then() chain or Sequelize query methods)
/// should be in a directory ending with "-async" to signal their async nature.
/// Applies only to files inside queries/, steps/, alerts/, or actions/ directories.
pub struct AsyncDirectoryName;

const SCORE_PER_VIOLATION: u32 = 50;

impl Rule for AsyncDirectoryName {
    fn name(&self) -> &str {
        "async-directory-name"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/async-directory-name.md"
    }

    fn description(&self) -> &str {
        "Promise-returning files should be in -async directories"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["queries/get-users/index.js returning Model.findAll().then(...)"],
            &["queries/get-users-async/index.js returning Model.findAll().then(...)"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        // Only check index.js files in relevant directories
        if !is_relevant_index_file(path) {
            return vec![];
        }

        // Skip if already in an -async directory
        if is_in_async_folder(path) {
            return vec![];
        }

        let score = config.rule_score("async-directory-name", SCORE_PER_VIOLATION);

        // Check if the default export returns a promise
        if returns_promise_chain(tree.root_node(), source) {
            let dir_name = path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "directory".to_string());

            return vec![RuleViolation {
                rule_name: self.name().to_string(),
                doc_url: self.doc_url().to_string(),
                score,
                code_sample: format!("{dir_name}/index.js returns a Promise"),
                fix_suggestion: format!("rename directory to {dir_name}-async"),
            }];
        }

        vec![]
    }
}

/// Check if file is index.js inside queries/, steps/, alerts/, or actions/.
fn is_relevant_index_file(path: &Path) -> bool {
    // Must be index.js
    if path.file_name().map(|n| n != "index.js").unwrap_or(true) {
        return false;
    }

    let path_str = path.to_string_lossy();

    // Must be inside one of these directories
    path_str.contains("/queries/")
        || path_str.contains("/steps/")
        || path_str.contains("/alerts/")
        || path_str.contains("/actions/")
}

/// Check if the file's immediate parent directory ends with "-async".
fn is_in_async_folder(path: &Path) -> bool {
    path.parent()
        .and_then(|p| p.file_name())
        .is_some_and(|name| name.to_string_lossy().ends_with("-async"))
}

/// Check if the default export function returns a promise chain.
fn returns_promise_chain(root: tree_sitter::Node, source: &str) -> bool {
    // Find default export
    let func = match find_default_export_function(root) {
        Some(f) => f,
        None => return false,
    };

    // Check if it returns a .then() chain or Sequelize query
    has_promise_return(func, source)
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

/// Check if a function has a return statement with a .then() chain or Sequelize query.
fn has_promise_return(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() == "return_statement" {
        let text = &source[node.byte_range()];
        // Check for .then() chain
        if text.contains(".then(") {
            return true;
        }
        // Check for Sequelize query methods
        if text.contains(".findAll(")
            || text.contains(".findOne(")
            || text.contains(".findByPk(")
            || text.contains(".create(")
            || text.contains(".update(")
            || text.contains(".destroy(")
            || text.contains(".count(")
            || text.contains(".bulkCreate(")
        {
            return true;
        }
    }

    // Don't recurse into nested functions
    if node.kind() == "function_declaration"
        || node.kind() == "function"
        || node.kind() == "arrow_function"
    {
        // Only continue if this is the top-level function
        if node.parent().is_some_and(|p| p.kind() != "export_statement" && p.kind() != "program") {
            return false;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_promise_return(child, source) {
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
        AsyncDirectoryName.check(source, Path::new("src/queries/get-users/index.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        AsyncDirectoryName.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_findall_then_in_queries() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
        assert!(violations[0].fix_suggestion.contains("-async"));
    }

    #[test]
    fn test_findone_then_in_queries() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: ctx.id } }).then((model) => ({ ...ctx, model }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_findall_without_then_in_queries() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} });
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_create_in_steps() {
        let source = r#"export default (ctx) => {
            return Model.create({ name: ctx.name }).then((model) => ({ ...ctx, model }));
        };"#;
        let violations = check_with_path(source, "src/steps/create-user/index.js");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_update_in_actions() {
        let source = r#"export default (ctx) => {
            return Model.update({ status: 'done' }, { where: { id: ctx.id } }).then(() => ctx);
        };"#;
        let violations = check_with_path(source, "src/actions/update-status/index.js");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_destroy_in_alerts() {
        let source = r#"export default (ctx) => {
            return Model.destroy({ where: { id: ctx.id } }).then(() => ctx);
        };"#;
        let violations = check_with_path(source, "src/alerts/delete-notification/index.js");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_then_chain_without_sequelize() {
        let source = r#"export default (ctx) => {
            return somePromise().then((result) => ({ ...ctx, result }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_already_async_directory() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check_with_path(source, "src/queries/get-users-async/index.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_index_js() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check_with_path(source, "src/queries/get-users/helper.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_relevant_directory() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check_with_path(source, "src/utils/get-users/index.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_sync_function() {
        let source = r#"export default (ctx) => {
            return { ...ctx, computed: ctx.value * 2 };
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_return() {
        let source = r#"export default (ctx) => {
            console.log(ctx);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_default_export() {
        let source = r#"export const helper = (ctx) => {
            return Model.findAll({ where: {} }).then((r) => r);
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_nested_function_with_then_not_flagged() {
        // The .then() is inside a nested function, not the main return
        let source = r#"export default (ctx) => {
            const helper = () => Model.findAll({}).then((r) => r);
            return { ...ctx, helper };
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_spec_file_excluded() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check_with_path(source, "src/queries/get-users/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_deeply_nested_queries_path() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((results) => ({ ...ctx, results }));
        };"#;
        let violations = check_with_path(source, "src/core/features/bonuses/queries/get-pending/index.js");
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let source = r#"export default (ctx) => {
            return Model.findAll({}).then((r) => ({ ...ctx, r }));
        };"#;
        let violations = check(source);
        assert_eq!(violations[0].rule_name, "async-directory-name");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let source = r#"export default (ctx) => {
            return Model.findAll({}).then((r) => ({ ...ctx, r }));
        };"#;
        let violations = check(source);
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_fix_suggestion_contains_directory_name() {
        let source = r#"export default (ctx) => {
            return Model.findAll({}).then((r) => ({ ...ctx, r }));
        };"#;
        let violations = check(source);
        assert!(violations[0].fix_suggestion.contains("get-users-async"));
    }
}
