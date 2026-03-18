use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: files with more than 200 lines accumulate stink.
/// +10 stink for every 50 lines above 200.
pub struct FileTooLong;

const THRESHOLD: u32 = 200;
const LINES_PER_POINT: u32 = 50;
const POINTS_PER_BUCKET: u32 = 10;

impl Rule for FileTooLong {
    fn name(&self) -> &str {
        "file-too-long"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/file-too-long.md"
    }

    fn check(&self, source: &str, _path: &Path, _tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let points_per_bucket = config.rule_score("file-too-long", POINTS_PER_BUCKET);
        let line_count = source.lines().count() as u32;

        if line_count <= THRESHOLD {
            return vec![];
        }

        let lines_over = line_count - THRESHOLD;
        let buckets = lines_over / LINES_PER_POINT;

        if buckets == 0 {
            return vec![];
        }

        let score = buckets * points_per_bucket;

        vec![RuleViolation {
            rule_name: self.name().to_string(),
            doc_url: self.doc_url().to_string(),
            score,
            code_sample: format!("{line_count} lines"),
            fix_suggestion: format!("split file into smaller modules (currently {line_count} lines, threshold {THRESHOLD})"),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn make_source(lines: u32) -> String {
        (0..lines).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
    }

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        FileTooLong.check(source, Path::new("test.js"), &tree, &mut cache, &config)
    }

    #[test]
    fn test_under_threshold_no_violations() {
        assert!(check(&make_source(100)).is_empty());
    }

    #[test]
    fn test_exactly_at_threshold_no_violations() {
        assert!(check(&make_source(200)).is_empty());
    }

    #[test]
    fn test_just_over_threshold_but_under_one_point() {
        // 230 lines = 30 over, 30/50 = 0 points
        assert!(check(&make_source(230)).is_empty());
    }

    #[test]
    fn test_one_bucket() {
        // 250 lines = 50 over, 1 bucket * 10 = 10
        let violations = check(&make_source(250));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_two_buckets() {
        // 300 lines = 100 over, 2 buckets * 10 = 20
        let violations = check(&make_source(300));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_six_buckets() {
        // 500 lines = 300 over, 6 buckets * 10 = 60
        let violations = check(&make_source(500));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_partial_bucket_rounds_down() {
        // 275 lines = 75 over, 1 bucket * 10 = 10 (rounds down)
        let violations = check(&make_source(275));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(&make_source(300));
        assert_eq!(violations[0].rule_name, "file-too-long");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(&make_source(300));
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_empty_file() {
        assert!(check("").is_empty());
    }

    #[test]
    fn test_one_line_file() {
        assert!(check("const x = 1;").is_empty());
    }
}
