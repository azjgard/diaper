use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: ternary operator usage adds stink.
/// Single ternary: +10. Nested ternary (more than one ? on a line): +60.
pub struct TernaryOperator;

const SINGLE_SCORE: u32 = 10;
const NESTED_SCORE: u32 = 60;

impl Rule for TernaryOperator {
    fn name(&self) -> &str {
        "ternary-operator"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/ternary-operator.md"
    }

    fn check(&self, source: &str, _path: &Path) -> Vec<RuleViolation> {
        let mut violations = Vec::new();

        for line in source.lines() {
            let count = count_ternaries(line);

            if count == 0 {
                continue;
            }

            if count == 1 {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: SINGLE_SCORE,
                    message: format!("ternary operator: {}", line.trim()),
                });
            } else {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: NESTED_SCORE,
                    message: format!("nested ternary ({count} levels): {}", line.trim()),
                });
            }
        }

        violations
    }
}

/// Count the number of ternary `?` operators in a line.
/// Skores `?.` (optional chaining) and `?` inside strings.
fn count_ternaries(line: &str) -> u32 {
    let mut count = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_template = false;
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        // Track string state (skip escaped quotes)
        if b == b'\\' && (in_single_quote || in_double_quote || in_template) {
            i += 2;
            continue;
        }

        if b == b'\'' && !in_double_quote && !in_template {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single_quote && !in_template {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }
        if b == b'`' && !in_single_quote && !in_double_quote {
            in_template = !in_template;
            i += 1;
            continue;
        }

        // Only count ? outside of strings
        if !in_single_quote && !in_double_quote && !in_template && b == b'?' {
            // Skip ?. (optional chaining) and ?? (nullish coalescing)
            if i + 1 < len && (bytes[i + 1] == b'.' || bytes[i + 1] == b'?') {
                i += 2;
                continue;
            }
            count += 1;
        }

        i += 1;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Vec<RuleViolation> {
        TernaryOperator.check(source, Path::new("src/foo.js"))
    }

    // --- Single ternary ---

    #[test]
    fn test_simple_ternary() {
        let violations = check("const x = a ? b : c;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_ternary_in_assignment() {
        let violations = check("const result = isReady ? 'yes' : 'no';");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    #[test]
    fn test_ternary_in_return() {
        let violations = check("return active ? doThis() : doThat();");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    // --- Nested ternary ---

    #[test]
    fn test_nested_ternary() {
        let violations = check("const x = a ? b ? c : d : e;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_double_nested_ternary() {
        let violations = check("const x = a ? b ? c ? d : e : f : g;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
        assert!(violations[0].message.contains("3 levels"));
    }

    #[test]
    fn test_nested_in_else_branch() {
        let violations = check("const x = a ? b : c ? d : e;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    // --- Multiple lines ---

    #[test]
    fn test_multiple_lines_with_ternaries() {
        let source = "const a = x ? 1 : 2;\nconst b = y ? 3 : 4;";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 20);
    }

    #[test]
    fn test_mix_single_and_nested() {
        let source = "const a = x ? 1 : 2;\nconst b = x ? y ? 1 : 2 : 3;";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].score, 10);
        assert_eq!(violations[1].score, 60);
    }

    // --- No violations ---

    #[test]
    fn test_no_ternary() {
        let violations = check("const x = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_optional_chaining_not_counted() {
        let violations = check("const x = foo?.bar?.baz;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_nullish_coalescing_not_counted() {
        let violations = check("const x = foo ?? bar;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_string_not_counted() {
        let violations = check(r#"const x = "is this a question?";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_single_quote_string() {
        let violations = check("const x = 'what?';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_question_mark_in_template_literal() {
        let violations = check("const x = `is this ${y}?`;");
        assert!(violations.is_empty());
    }

    // --- Mixed with optional chaining ---

    #[test]
    fn test_ternary_with_optional_chaining() {
        // One real ternary + optional chaining should be 1 ternary
        let violations = check("const x = foo?.bar ? 'yes' : 'no';");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    // --- count_ternaries unit tests ---

    #[test]
    fn test_count_zero() {
        assert_eq!(count_ternaries("const x = 42;"), 0);
    }

    #[test]
    fn test_count_one() {
        assert_eq!(count_ternaries("a ? b : c"), 1);
    }

    #[test]
    fn test_count_two() {
        assert_eq!(count_ternaries("a ? b ? c : d : e"), 2);
    }

    #[test]
    fn test_count_skips_optional_chaining() {
        assert_eq!(count_ternaries("foo?.bar"), 0);
    }

    #[test]
    fn test_count_skips_nullish_coalescing() {
        assert_eq!(count_ternaries("foo ?? bar"), 0);
    }

    #[test]
    fn test_count_skips_string() {
        assert_eq!(count_ternaries(r#""what?""#), 0);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("const x = a ? b : c;");
        assert_eq!(violations[0].rule_name, "ternary-operator");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("const x = a ? b : c;");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
