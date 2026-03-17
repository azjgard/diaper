pub mod async_await;
pub mod file_too_long;
pub mod non_default_export;
pub mod ternary_operator;
pub mod upward_relative_import;

use std::path::Path;

/// A single violation found by a rule.
pub struct RuleViolation {
    /// Name of the rule that was violated.
    pub rule_name: String,
    /// Link to documentation explaining the rule and how to fix it.
    pub doc_url: String,
    /// How much stink this violation adds to the file score.
    pub score: u32,
    /// Human-readable explanation of the violation.
    pub message: String,
}

/// A rule that can score a JavaScript file for code smells.
pub trait Rule {
    /// Short name for this rule (e.g. "file-too-long").
    fn name(&self) -> &str;

    /// URL linking to documentation about this rule.
    fn doc_url(&self) -> &str;

    /// Score the given file. Returns zero or more violations.
    /// `source` is the file contents, `path` is the file path,
    /// `tree` is the tree-sitter parse tree for the file.
    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree) -> Vec<RuleViolation>;
}

/// Parse JavaScript source into a tree-sitter tree.
pub fn parse_js(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).ok()?;
    parser.parse(source, None)
}

/// Returns all registered rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(async_await::AsyncAwait),
        Box::new(file_too_long::FileTooLong),
        Box::new(non_default_export::NonDefaultExport),
        Box::new(ternary_operator::TernaryOperator),
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
}
