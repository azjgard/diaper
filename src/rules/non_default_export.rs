use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: functions that are exported as named (non-default) exports add 50 stink each.
/// Encourages single-responsibility files with one default export.
pub struct NonDefaultExport;

const SCORE_PER_VIOLATION: u32 = 50;

impl Rule for NonDefaultExport {
    fn name(&self) -> &str {
        "non-default-export"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/non-default-export.md"
    }

    fn check(&self, source: &str, _path: &Path) -> Vec<RuleViolation> {
        let mut violations = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            if is_named_function_export(trimmed) {
                let fn_name = extract_function_name(trimmed).unwrap_or("anonymous");
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score: SCORE_PER_VIOLATION,
                    message: format!("non-default exported function: {fn_name}"),
                });
            }
        }

        violations
    }
}

/// Returns true if the line is a named (non-default) function export.
fn is_named_function_export(line: &str) -> bool {
    // Must start with "export " but NOT "export default"
    let after_export = match line.strip_prefix("export ") {
        Some(rest) => rest,
        None => return false,
    };

    if after_export.starts_with("default ") {
        return false;
    }

    // Match: export function, export async function, export const/let/var ... = function/arrow
    if after_export.starts_with("function ")
        || after_export.starts_with("async function ")
    {
        return true;
    }

    // Match: export const foo = (...) => or export const foo = function
    if after_export.starts_with("const ")
        || after_export.starts_with("let ")
        || after_export.starts_with("var ")
    {
        if let Some(after_eq) = after_export.split_once('=') {
            let rhs = after_eq.1.trim();
            if rhs.starts_with("function")
                || rhs.starts_with("async")
                || rhs.starts_with('(')
                || rhs.contains("=>")
            {
                return true;
            }
        }
    }

    false
}

/// Extract the function name from an export line.
fn extract_function_name(line: &str) -> Option<&str> {
    let after_export = line.strip_prefix("export ")?;

    // "export function foo(" or "export async function foo("
    let after_fn = if let Some(rest) = after_export.strip_prefix("async function ") {
        rest
    } else if let Some(rest) = after_export.strip_prefix("function ") {
        rest
    } else {
        // "export const foo = ..."
        let after_keyword = after_export
            .strip_prefix("const ")
            .or_else(|| after_export.strip_prefix("let "))
            .or_else(|| after_export.strip_prefix("var "))?;
        let end = after_keyword.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')?;
        return Some(&after_keyword[..end]);
    };

    let end = after_fn.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')?;
    Some(&after_fn[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Vec<RuleViolation> {
        NonDefaultExport.check(source, Path::new("src/foo.js"))
    }

    // --- Violations ---

    #[test]
    fn test_export_function() {
        let violations = check("export function foo() {}");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 50);
    }

    #[test]
    fn test_export_async_function() {
        let violations = check("export async function fetchData() {}");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_arrow() {
        let violations = check("export const foo = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_arrow_with_params() {
        let violations = check("export const foo = (a, b) => a + b;");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_function_expression() {
        let violations = check("export const foo = function() {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_const_async_arrow() {
        let violations = check("export const foo = async () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_let_arrow() {
        let violations = check("export let handler = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_export_var_arrow() {
        let violations = check("export var handler = () => {};");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_exports() {
        let source = "export function foo() {}\nexport function bar() {}\nexport const baz = () => {};";
        let violations = check(source);
        assert_eq!(violations.len(), 3);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 150);
    }

    // --- OK (no violations) ---

    #[test]
    fn test_default_export_function() {
        let violations = check("export default function foo() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_export_anonymous() {
        let violations = check("export default function() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_default_export_expression() {
        let violations = check("export default () => {};");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_export() {
        let violations = check("function foo() {}");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_non_function() {
        // Not a function — just a value
        let violations = check("export const FOO = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_string() {
        let violations = check("export const NAME = 'hello';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_export_const_object() {
        let violations = check("export const config = { port: 3000 };");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_exports() {
        let violations = check("const x = 1;\nfunction foo() {}\nconsole.log(foo());");
        assert!(violations.is_empty());
    }

    // --- Function name extraction ---

    #[test]
    fn test_message_contains_function_name() {
        let violations = check("export function myHelper() {}");
        assert!(violations[0].message.contains("myHelper"));
    }

    #[test]
    fn test_message_contains_const_name() {
        let violations = check("export const doStuff = () => {};");
        assert!(violations[0].message.contains("doStuff"));
    }

    #[test]
    fn test_message_contains_async_function_name() {
        let violations = check("export async function loadData() {}");
        assert!(violations[0].message.contains("loadData"));
    }

    // --- extract_function_name unit tests ---

    #[test]
    fn test_extract_name_function() {
        assert_eq!(extract_function_name("export function foo() {}"), Some("foo"));
    }

    #[test]
    fn test_extract_name_async_function() {
        assert_eq!(extract_function_name("export async function bar() {}"), Some("bar"));
    }

    #[test]
    fn test_extract_name_const() {
        assert_eq!(extract_function_name("export const baz = () => {}"), Some("baz"));
    }

    #[test]
    fn test_extract_name_not_export() {
        assert_eq!(extract_function_name("function foo() {}"), None);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check("export function foo() {}");
        assert_eq!(violations[0].rule_name, "non-default-export");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check("export function foo() {}");
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
