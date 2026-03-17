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

    fn check(&self, source: &str, _path: &Path) -> Vec<RuleViolation> {
        let line_count = source.lines().count() as u32;

        if line_count <= THRESHOLD {
            return vec![];
        }

        let lines_over = line_count - THRESHOLD;
        let buckets = lines_over / LINES_PER_POINT;

        if buckets == 0 {
            return vec![];
        }

        let score = buckets * POINTS_PER_BUCKET;

        vec![RuleViolation {
            rule_name: self.name().to_string(),
            doc_url: self.doc_url().to_string(),
            score,
            message: format!("file is {line_count} lines (threshold: {THRESHOLD}, +{POINTS_PER_BUCKET} per {LINES_PER_POINT} lines over)"),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(lines: u32) -> String {
        (0..lines).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn test_under_threshold_no_violations() {
        let rule = FileTooLong;
        let source = make_source(100);
        let violations = rule.check(&source, Path::new("test.js"));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_exactly_at_threshold_no_violations() {
        let rule = FileTooLong;
        let source = make_source(200);
        let violations = rule.check(&source, Path::new("test.js"));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_just_over_threshold_but_under_one_point() {
        let rule = FileTooLong;
        // 230 lines = 30 over, 30/50 = 0 points
        let source = make_source(230);
        let violations = rule.check(&source, Path::new("test.js"));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_one_bucket() {
        let rule = FileTooLong;
        // 250 lines = 50 over, 1 bucket * 10 = 10
        let source = make_source(250);
        let violations = rule.check(&source, Path::new("test.js"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_two_buckets() {
        let rule = FileTooLong;
        // 300 lines = 100 over, 2 buckets * 10 = 20
        let source = make_source(300);
        let violations = rule.check(&source, Path::new("test.js"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 20);
    }

    #[test]
    fn test_six_buckets() {
        let rule = FileTooLong;
        // 500 lines = 300 over, 6 buckets * 10 = 60
        let source = make_source(500);
        let violations = rule.check(&source, Path::new("test.js"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_partial_bucket_rounds_down() {
        let rule = FileTooLong;
        // 275 lines = 75 over, 1 bucket * 10 = 10 (rounds down)
        let source = make_source(275);
        let violations = rule.check(&source, Path::new("test.js"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_violation_has_correct_rule_name() {
        let rule = FileTooLong;
        let source = make_source(300);
        let violations = rule.check(&source, Path::new("test.js"));
        assert_eq!(violations[0].rule_name, "file-too-long");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let rule = FileTooLong;
        let source = make_source(300);
        let violations = rule.check(&source, Path::new("test.js"));
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_empty_file() {
        let rule = FileTooLong;
        let violations = rule.check("", Path::new("test.js"));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_one_line_file() {
        let rule = FileTooLong;
        let violations = rule.check("const x = 1;", Path::new("test.js"));
        assert!(violations.is_empty());
    }
}
