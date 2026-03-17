use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: imports using relative paths that go up the directory hierarchy
/// (starting with "../") are a code smell unless the path contains "shared".
/// Each violating import adds 100 stink.
pub struct UpwardRelativeImport;

const SCORE_PER_VIOLATION: u32 = 100;

impl Rule for UpwardRelativeImport {
    fn name(&self) -> &str {
        "upward-relative-import"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/upward-relative-import.md"
    }

    fn check(&self, source: &str, _path: &Path) -> Vec<RuleViolation> {
        let mut violations = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            let import_path = match extract_import_path(trimmed) {
                Some(p) => p,
                None => continue,
            };

            if !import_path.starts_with("../") {
                continue;
            }

            if import_path.contains("shared") {
                continue;
            }

            violations.push(RuleViolation {
                rule_name: self.name().to_string(),
                doc_url: self.doc_url().to_string(),
                score: SCORE_PER_VIOLATION,
                message: format!("upward relative import: \"{import_path}\""),
            });
        }

        violations
    }
}

/// Extract the import path string from an import or require statement.
/// Returns None if the line is not an import/require.
fn extract_import_path(line: &str) -> Option<&str> {
    // Handle: import ... from "path" or import ... from 'path'
    if line.starts_with("import ") {
        if let Some(path) = extract_string_after(line, " from ") {
            return Some(path);
        }
        // Handle: import "path" or import 'path' (side-effect imports)
        return extract_string_from_import(line);
    }

    // Handle: require("path") or require('path')
    if let Some(start) = line.find("require(") {
        let after = &line[start + 8..];
        return extract_quoted_string(after);
    }

    None
}

/// Extract a quoted string after a delimiter like " from ".
fn extract_string_after<'a>(line: &'a str, delimiter: &str) -> Option<&'a str> {
    let idx = line.find(delimiter)?;
    let after = &line[idx + delimiter.len()..];
    extract_quoted_string(after)
}

/// Extract a quoted string value (the content between matching quotes).
fn extract_quoted_string(s: &str) -> Option<&str> {
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[1..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

/// Handle side-effect imports like: import "path" or import 'path'
fn extract_string_from_import(line: &str) -> Option<&str> {
    let after_import = line.strip_prefix("import ")?;
    let trimmed = after_import.trim();
    extract_quoted_string(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Vec<RuleViolation> {
        UpwardRelativeImport.check(source, Path::new("test.js"))
    }

    // --- Violations (should produce stink) ---

    #[test]
    fn test_upward_relative_import_double_quotes() {
        let violations = check(r#"import x from "../../src";"#);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_upward_relative_import_single_quotes() {
        let violations = check("import x from '../utils';");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_upward_relative_import_deeply_nested() {
        let violations = check(r#"import { foo } from "../../../deeply/nested/thing";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_multiple_upward_imports() {
        let source = r#"
import a from "../foo";
import b from "../bar";
import c from "../baz";
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 3);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 300);
    }

    #[test]
    fn test_require_upward_relative() {
        let violations = check(r#"const x = require("../foo");"#);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_require_single_quotes_upward() {
        let violations = check("const x = require('../foo');");
        assert_eq!(violations.len(), 1);
    }

    // --- OK (should NOT produce stink) ---

    #[test]
    fn test_same_dir_relative_import_ok() {
        let violations = check(r#"import x from "./src";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_same_dir_single_quotes_ok() {
        let violations = check("import x from './src';");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_alias_import_ok() {
        let violations = check(r##"import x from "#some-alias/src";"##);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_package_import_ok() {
        let violations = check(r#"import React from "react";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_shared_in_path_ok() {
        let violations = check(r#"import x from "../../shared/utils";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_shared_deeper_in_path_ok() {
        let violations = check(r#"import x from "../../../lib/shared/helpers";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_require_same_dir_ok() {
        let violations = check(r#"const x = require("./foo");"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_require_package_ok() {
        let violations = check(r#"const x = require("lodash");"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_require_shared_ok() {
        let violations = check(r#"const x = require("../../shared/config");"#);
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_imports() {
        let violations = check("const x = 42;\nconsole.log(x);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_side_effect_upward_import() {
        let violations = check(r#"import "../polyfill";"#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_side_effect_same_dir_ok() {
        let violations = check(r#"import "./polyfill";"#);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mixed_imports() {
        let source = r#"
import React from "react";
import x from "./local";
import y from "../../bad";
import z from "../../shared/ok";
const w = require("../also-bad");
const v = require("./fine");
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2); // "../../bad" and "../also-bad"
    }

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(r#"import x from "../foo";"#);
        assert_eq!(violations[0].rule_name, "upward-relative-import");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(r#"import x from "../foo";"#);
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    #[test]
    fn test_violation_message_contains_path() {
        let violations = check(r#"import x from "../../src";"#);
        assert!(violations[0].message.contains("../../src"));
    }

    // --- extract_import_path unit tests ---

    #[test]
    fn test_extract_import_path_from_import() {
        assert_eq!(extract_import_path(r#"import x from "../foo""#), Some("../foo"));
    }

    #[test]
    fn test_extract_import_path_from_require() {
        assert_eq!(extract_import_path(r#"const x = require("../foo")"#), Some("../foo"));
    }

    #[test]
    fn test_extract_import_path_not_import() {
        assert_eq!(extract_import_path("const x = 42;"), None);
    }

    #[test]
    fn test_extract_import_path_side_effect() {
        assert_eq!(extract_import_path(r#"import "../polyfill""#), Some("../polyfill"));
    }
}
