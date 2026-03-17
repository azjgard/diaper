use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: usage of `async` or `await` keywords adds 100 stink per occurrence,
/// unless the file is named "index.spec.js" or the path contains "/migrations".
pub struct AsyncAwait;

const SCORE_PER_VIOLATION: u32 = 100;

impl Rule for AsyncAwait {
    fn name(&self) -> &str {
        "async-await"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/async-await.md"
    }

    fn check(&self, source: &str, path: &Path) -> Vec<RuleViolation> {
        let path_str = path.to_string_lossy();

        // Skip index.spec.js files
        if path.file_name().is_some_and(|f| f == "index.spec.js") {
            return vec![];
        }

        // Skip migration files
        if path_str.contains("/migrations") {
            return vec![];
        }

        let mut violations = Vec::new();

        for line in source.lines() {
            let count = count_async_await(line);
            for _ in 0..count {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: SCORE_PER_VIOLATION,
                    message: format!("async/await usage: {}", line.trim()),
                });
            }
        }

        violations
    }
}

/// Count occurrences of `async` or `await` as whole words in a line.
fn count_async_await(line: &str) -> u32 {
    let mut count = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();

    for keyword in &["async", "await"] {
        let kw_bytes = keyword.as_bytes();
        let kw_len = kw_bytes.len();

        let mut i = 0;
        while i + kw_len <= len {
            if &bytes[i..i + kw_len] == kw_bytes {
                let before_ok = i == 0 || !is_word_char(bytes[i - 1]);
                let after_ok = i + kw_len >= len || !is_word_char(bytes[i + kw_len]);
                if before_ok && after_ok {
                    count += 1;
                    i += kw_len;
                    continue;
                }
            }
            i += 1;
        }
    }

    count
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Vec<RuleViolation> {
        AsyncAwait.check(source, Path::new("src/foo.js"))
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        AsyncAwait.check(source, Path::new(path))
    }

    // --- Violations ---

    #[test]
    fn test_async_function() {
        let violations = check("async function foo() {}");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_await_expression() {
        let violations = check("const x = await fetch('/api');");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_async_arrow() {
        let violations = check("const foo = async () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_async_and_await_same_line() {
        let violations = check("const x = async () => await fetch('/api');");
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_multiple_lines() {
        let source = "async function foo() {\n  const x = await bar();\n}";
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 200);
    }

    #[test]
    fn test_multiple_awaits_same_line() {
        let violations = check("const [a, b] = await Promise.all([await foo(), await bar()]);");
        assert_eq!(violations.len(), 3);
    }

    // --- Excluded paths ---

    #[test]
    fn test_index_spec_js_excluded() {
        let violations = check_with_path("async function foo() {}", "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_index_spec_js_nested_excluded() {
        let violations = check_with_path("async function foo() {}", "packages/app/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_path_excluded() {
        let violations = check_with_path("async function foo() {}", "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_migrations_nested_excluded() {
        let violations = check_with_path("async function foo() {}", "db/migrations/seed.js");
        assert!(violations.is_empty());
    }

    // --- NOT excluded ---

    #[test]
    fn test_other_spec_file_not_excluded() {
        let violations = check_with_path("async function foo() {}", "src/foo.spec.js");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_regular_file_not_excluded() {
        let violations = check_with_path("async function foo() {}", "src/service.js");
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_no_async_await() {
        let violations = check("function foo() { return 1; }");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_in_variable_name_no_match() {
        // "asyncFoo" should not match — not a whole word
        let violations = check("const asyncFoo = 1;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_await_in_variable_name_no_match() {
        let violations = check("const awaitResult = 1;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_async_in_string_matches() {
        // We do simple text matching — strings count too
        let violations = check(r#"const x = "async";"#);
        assert_eq!(violations.len(), 1);
    }

    // --- count_async_await unit tests ---

    #[test]
    fn test_count_none() {
        assert_eq!(count_async_await("const x = 1;"), 0);
    }

    #[test]
    fn test_count_one_async() {
        assert_eq!(count_async_await("async function foo() {}"), 1);
    }

    #[test]
    fn test_count_one_await() {
        assert_eq!(count_async_await("const x = await foo();"), 1);
    }

    #[test]
    fn test_count_both() {
        assert_eq!(count_async_await("async () => await foo()"), 2);
    }

    #[test]
    fn test_count_no_partial_match() {
        assert_eq!(count_async_await("asyncFoo awaitBar"), 0);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("async function foo() {}");
        assert_eq!(violations[0].rule_name, "async-await");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("async function foo() {}");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
