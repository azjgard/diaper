use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: ternary operator usage adds stink.
/// Single ternary: +10. Nested/multiple ternaries in one expression: +60.
/// Counts across the whole file to catch multi-line ternaries.
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

        // Find all ternary expressions by tracking ? and : depth across the source.
        // We group ternaries into "expressions" separated by semicolons/statement boundaries.
        let expressions = find_ternary_expressions(source);

        for expr in expressions {
            if expr.ternary_count == 1 {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: SINGLE_SCORE,
                    message: format!("ternary operator: {}", expr.snippet),
                });
            } else {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: NESTED_SCORE,
                    message: format!("nested ternary ({} levels): {}", expr.ternary_count, expr.snippet),
                });
            }
        }

        violations
    }
}

struct TernaryExpression {
    ternary_count: u32,
    snippet: String,
}

/// Scan the source for ternary expressions, grouping consecutive ternaries
/// that belong to the same statement (separated by ; or statement boundaries).
fn find_ternary_expressions(source: &str) -> Vec<TernaryExpression> {
    let mut results = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_template = false;

    // Track current statement's ternary count and the line of the first ?
    let mut current_count: u32 = 0;
    let mut first_ternary_line: Option<usize> = None;

    let lines: Vec<&str> = source.lines().collect();
    let mut current_line: usize = 0;

    let mut i = 0;
    while i < len {
        let b = bytes[i];

        // Track line numbers
        if b == b'\n' {
            current_line += 1;
            i += 1;
            continue;
        }

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

        let in_string = in_single_quote || in_double_quote || in_template;

        if !in_string {
            if b == b'?' {
                // Skip ?. (optional chaining) and ?? (nullish coalescing)
                if i + 1 < len && (bytes[i + 1] == b'.' || bytes[i + 1] == b'?') {
                    i += 2;
                    continue;
                }
                current_count += 1;
                if first_ternary_line.is_none() {
                    first_ternary_line = Some(current_line);
                }
            }

            // Statement boundary: ; or { or } flush the current expression
            if b == b';' || b == b'{' || b == b'}' {
                if current_count > 0 {
                    let line_idx = first_ternary_line.unwrap_or(0);
                    let snippet = lines.get(line_idx).unwrap_or(&"").trim().to_string();
                    results.push(TernaryExpression {
                        ternary_count: current_count,
                        snippet,
                    });
                    current_count = 0;
                    first_ternary_line = None;
                }
            }
        }

        i += 1;
    }

    // Flush any remaining expression at end of file
    if current_count > 0 {
        let line_idx = first_ternary_line.unwrap_or(0);
        let snippet = lines.get(line_idx).unwrap_or(&"").trim().to_string();
        results.push(TernaryExpression {
            ternary_count: current_count,
            snippet,
        });
    }

    results
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

    // --- Nested ternary (single line) ---

    #[test]
    fn test_nested_ternary_single_line() {
        let violations = check("const x = a ? b ? c : d : e;");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
    }

    #[test]
    fn test_triple_nested_ternary() {
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

    // --- Nested ternary (multi-line) ---

    #[test]
    fn test_multiline_nested_ternary() {
        let source = r#"  const tern = true
    ? (await fetch("/api"))
      ? console.log("do something totally crazy")
      : false
    : false;"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 60);
        assert!(violations[0].message.contains("2 levels"));
    }

    #[test]
    fn test_multiline_single_ternary() {
        let source = "const x = condition\n  ? valueA\n  : valueB;";
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
    }

    // --- Multiple separate ternaries ---

    #[test]
    fn test_two_separate_ternaries() {
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
        let violations = check("const x = foo?.bar ? 'yes' : 'no';");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
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
