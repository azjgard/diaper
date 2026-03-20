pub mod async_await;
pub mod async_promise_return;
pub mod ctx_destructure;
pub mod file_too_long;
pub mod graphql_type_export;
pub mod non_default_export;
pub mod non_idempotent_migration;
pub mod pipe_property_init;
pub mod reduce_param_name;
pub mod require_query_attributes;
pub mod short_iter_param;
pub mod ternary_operator;
pub mod unsorted_string_array;
pub mod upward_relative_import;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

/// A single violation found by a rule.
pub struct RuleViolation {
    /// Name of the rule that was violated.
    pub rule_name: String,
    /// Link to documentation explaining the rule and how to fix it.
    pub doc_url: String,
    /// How much stink this violation adds to the file score.
    pub score: u32,
    /// Code sample or context for the violation.
    pub code_sample: String,
    /// Succinct suggestion for how to fix the violation.
    pub fix_suggestion: String,
}

/// A rule that can score a JavaScript file for code smells.
pub trait Rule {
    /// Short name for this rule (e.g. "file-too-long").
    fn name(&self) -> &str;

    /// URL linking to documentation about this rule.
    fn doc_url(&self) -> &str;

    /// Score the given file. Returns zero or more violations.
    /// `source` is the file contents, `path` is the file path,
    /// `tree` is the tree-sitter parse tree for the file,
    /// `cache` allows rules to parse and access other files' ASTs on demand.
    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, cache: &mut AstCache, config: &Config) -> Vec<RuleViolation>;
}

/// Returns true if the file should be skipped by most rules.
/// Skips index.spec.js files and files in /migrations/ paths.
pub fn is_excluded_file(path: &Path) -> bool {
    if path.file_name().is_some_and(|f| f == "index.spec.js") {
        return true;
    }
    let path_str = path.to_string_lossy();
    if path_str.contains("/migrations") {
        return true;
    }
    false
}

/// Parse JavaScript source into a tree-sitter tree.
pub fn parse_js(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).ok()?;
    parser.parse(source, None)
}

/// Cache of parsed JavaScript ASTs. Stores source + tree per file path.
/// Rules can use this to parse and access other files on demand.
pub struct AstCache {
    entries: HashMap<PathBuf, (String, tree_sitter::Tree)>,
}

impl AstCache {
    pub fn new() -> Self {
        AstCache {
            entries: HashMap::new(),
        }
    }

    /// Pre-seed the cache with an already-parsed file.
    pub fn insert(&mut self, path: PathBuf, source: String, tree: tree_sitter::Tree) {
        self.entries.insert(path, (source, tree));
    }

    /// Get source + tree for a file, parsing and caching it on demand if needed.
    /// Returns None if the file can't be read or parsed.
    pub fn get_or_parse(&mut self, path: &Path) -> Option<&(String, tree_sitter::Tree)> {
        let abs = fs::canonicalize(path).ok()?;

        if !self.entries.contains_key(&abs) {
            let source = fs::read_to_string(&abs).ok()?;
            let tree = parse_js(&source)?;
            self.entries.insert(abs.clone(), (source, tree));
        }

        self.entries.get(&abs)
    }
}

/// Returns all registered rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(async_await::AsyncAwait),
        Box::new(async_promise_return::AsyncPromiseReturn),
        Box::new(ctx_destructure::CtxDestructure),
        Box::new(file_too_long::FileTooLong),
        Box::new(graphql_type_export::GraphqlTypeExport),
        Box::new(non_default_export::NonDefaultExport),
        Box::new(non_idempotent_migration::NonIdempotentMigration),
        Box::new(pipe_property_init::PipePropertyInit),
        Box::new(reduce_param_name::ReduceParamName),
        Box::new(require_query_attributes::RequireQueryAttributes),
        Box::new(short_iter_param::ShortIterParam),
        Box::new(ternary_operator::TernaryOperator),
        Box::new(unsorted_string_array::UnsortedStringArray),
        Box::new(upward_relative_import::UpwardRelativeImport),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_rules_returns_at_least_one() {
        let rules = all_rules();
        assert!(!rules.is_empty());
    }

    #[test]
    fn test_all_rules_have_names() {
        for rule in all_rules() {
            assert!(!rule.name().is_empty());
        }
    }

    #[test]
    fn test_all_rules_have_doc_urls() {
        for rule in all_rules() {
            assert!(!rule.doc_url().is_empty());
            assert!(rule.doc_url().starts_with("https://"));
        }
    }

    #[test]
    fn test_parse_js_valid() {
        let tree = parse_js("const x = 1;");
        assert!(tree.is_some());
    }

    #[test]
    fn test_parse_js_empty() {
        let tree = parse_js("");
        assert!(tree.is_some());
    }

    #[test]
    fn test_ast_cache_new() {
        let cache = AstCache::new();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_ast_cache_get_or_parse_missing_file() {
        let mut cache = AstCache::new();
        let result = cache.get_or_parse(Path::new("/tmp/nonexistent_diaper_cache_test.js"));
        assert!(result.is_none());
    }

    #[test]
    fn test_ast_cache_insert_and_retrieve() {
        let mut cache = AstCache::new();
        let source = "const x = 1;".to_string();
        let tree = parse_js(&source).unwrap();
        let path = PathBuf::from("/tmp/test_cache_insert.js");
        cache.insert(path.clone(), source, tree);
        assert!(cache.entries.contains_key(&path));
    }

    #[test]
    fn test_ast_cache_caches_on_reparse() {
        let mut cache = AstCache::new();
        // Write a temp file
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.js");
        fs::write(&file_path, "const x = 1;").unwrap();

        // First call parses
        let result1 = cache.get_or_parse(&file_path);
        assert!(result1.is_some());

        // Second call hits cache (same pointer = same entry)
        let result2 = cache.get_or_parse(&file_path);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().0, "const x = 1;");
    }
}
