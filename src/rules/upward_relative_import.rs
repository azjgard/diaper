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

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("upward-relative-import", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        collect_imports(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Walk the AST and find import statements and require() calls.
fn collect_imports(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &UpwardRelativeImport,
    score: u32,
) {
    match node.kind() {
        "import_statement" => {
            if let Some(path) = extract_import_source(node, source) {
                check_path(path, violations, rule, score);
            }
        }
        "call_expression" => {
            if let Some(path) = extract_require_source(node, source) {
                check_path(path, violations, rule, score);
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imports(child, source, violations, rule, score);
    }
}
fn check_path(path: &str, violations: &mut Vec<RuleViolation>, rule: &UpwardRelativeImport, score: u32) {
    if path.starts_with("../") && !path.contains("shared") {
        violations.push(RuleViolation {
            rule_name: rule.name().to_string(),
            doc_url: rule.doc_url().to_string(),
            score,
            code_sample: format!("import ... from \"{path}\""),
            fix_suggestion: format!("use an alias or move the import to a shared module instead of \"{path}\""),
        });
    }
}

/// Extract the source string from an import_statement node.
fn extract_import_source<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            return extract_string_content(child, source);
        }
    }
    None
}

/// Extract the source string from a require("...") call.
fn extract_require_source<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let func = node.child_by_field_name("function")?;
    let func_text = &source[func.byte_range()];
    if func_text != "require" {
        return None;
    }

    let args = node.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() == "string" {
            return extract_string_content(child, source);
        }
    }
    None
}

/// Extract the content of a string node (without quotes).
fn extract_string_content<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_fragment" {
            return Some(&source[child.byte_range()]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        UpwardRelativeImport.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
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
    fn test_import_in_comment_not_counted() {
        let violations = check(r#"// import x from "../foo";"#);
        assert!(violations.is_empty());
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
        assert!(violations[0].code_sample.contains("../../src"));
    }
}
