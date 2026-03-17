use std::fs;
use std::path::Path;

use crate::rules::{self, RuleViolation};

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

/// Check multiple files and print results.
pub fn check_files(paths: &[String]) -> Result<(), String> {
    let js_paths: Vec<&String> = paths.iter()
        .filter(|p| p.ends_with(".js"))
        .collect();

    if js_paths.is_empty() {
        println!("no JavaScript files to check");
        return Ok(());
    }

    let mut any_smells = false;

    for path in &js_paths {
        let result = check_file(path)?;

        if result.total_score > 0 {
            any_smells = true;
            println!("{} (score: {})", result.path, result.total_score);
            for violation in &result.violations {
                println!("  {} [+{}] {}", violation.rule_name, violation.score, violation.message);
                println!("    docs: {}", violation.doc_url);
            }
        }
    }

    if !any_smells {
        println!("all clean! no smells detected.");
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
        assert_eq!(result.total_score, 2);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_check_file_nonexistent() {
        let result = check_file("/tmp/nonexistent_diaper_test.js");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_files_filters_non_js() {
        // Should not error on non-js files, just skip them
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
}
