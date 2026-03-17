use std::fs;
use std::path::Path;

use crate::rules::{self, RuleViolation};

// ANSI color codes
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Result of checking a single file.
pub struct FileResult {
    pub path: String,
    pub total_score: u32,
    pub violations: Vec<RuleViolation>,
}

/// Check a single file against all rules.
pub fn check_file(path: &str) -> Result<FileResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {path}: {e}"))?;

    let rules = rules::all_rules();
    let file_path = Path::new(path);

    let mut violations = Vec::new();
    for rule in &rules {
        let mut rule_violations = rule.check(&source, file_path);
        violations.append(&mut rule_violations);
    }

    let total_score: u32 = violations.iter().map(|v| v.score).sum();

    Ok(FileResult {
        path: path.to_string(),
        total_score,
        violations,
    })
}

/// Format a URL as a clickable terminal hyperlink (OSC 8).
/// Falls back to just the visible text if the terminal doesn't support it.
fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

/// Pick a color for the score based on severity.
fn score_color(score: u32) -> &'static str {
    match score {
        0 => GREEN,
        1..=3 => YELLOW,
        _ => RED,
    }
}

/// Check multiple files and print results.
pub fn check_files(paths: &[String]) -> Result<(), String> {
    let js_paths: Vec<&String> = paths.iter()
        .filter(|p| p.ends_with(".js"))
        .collect();

    if js_paths.is_empty() {
        println!("{DIM}no JavaScript files to check{RESET}");
        return Ok(());
    }

    let mut any_smells = false;

    for path in &js_paths {
        let result = check_file(path)?;

        if result.total_score > 0 {
            any_smells = true;
            let color = score_color(result.total_score);
            println!("{BOLD}{}{RESET}  {color}stink: {}{RESET}", result.path, result.total_score);
            for violation in &result.violations {
                let doc_link = hyperlink(&violation.doc_url, "docs");
                println!("  {YELLOW}+{}{RESET}  {DIM}{}{RESET}  {}  {DIM}{doc_link}{RESET}", violation.score, violation.rule_name, violation.message);
            }
            println!();
        }
    }

    if !any_smells {
        println!("{GREEN}All clean. ✅{RESET}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_js_file(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(".js")
            .tempfile()
            .unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    fn make_js_source(lines: u32) -> String {
        (0..lines).map(|i| format!("const x{i} = {i};")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn test_check_file_short_file() {
        let file = create_temp_js_file("const x = 1;\n");
        let result = check_file(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.total_score, 0);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_check_file_long_file() {
        let source = make_js_source(300);
        let file = create_temp_js_file(&source);
        let result = check_file(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.total_score, 20);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_check_file_nonexistent() {
        let result = check_file("/tmp/nonexistent_diaper_test.js");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_files_filters_non_js() {
        let files = vec!["foo.rs".to_string(), "bar.py".to_string()];
        let result = check_files(&files);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_files_with_js_file() {
        let source = make_js_source(10);
        let file = create_temp_js_file(&source);
        let path = file.path().to_str().unwrap().to_string();
        let result = check_files(&[path]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hyperlink_format() {
        let link = hyperlink("https://example.com", "click me");
        assert!(link.contains("https://example.com"));
        assert!(link.contains("click me"));
        // Should contain OSC 8 escape sequences
        assert!(link.contains("\x1b]8;;"));
    }

    #[test]
    fn test_score_color_zero_is_green() {
        assert_eq!(score_color(0), GREEN);
    }

    #[test]
    fn test_score_color_low_is_yellow() {
        assert_eq!(score_color(1), YELLOW);
        assert_eq!(score_color(2), YELLOW);
        assert_eq!(score_color(3), YELLOW);
    }

    #[test]
    fn test_score_color_high_is_red() {
        assert_eq!(score_color(4), RED);
        assert_eq!(score_color(10), RED);
        assert_eq!(score_color(100), RED);
    }
}
