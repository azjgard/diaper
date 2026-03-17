pub mod file_too_long;

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
    /// `source` is the file contents, `path` is the file path.
    fn check(&self, source: &str, path: &Path) -> Vec<RuleViolation>;
}

/// Returns all registered rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(file_too_long::FileTooLong),
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
}
