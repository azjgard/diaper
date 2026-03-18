use std::fs;
use std::path::Path;

use crate::rules::{self, AstCache, RuleViolation};

// ANSI color codes
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const BRIGHT_RED: &str = "\x1b[91m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Result of checking a single file.
pub struct FileResult {
    pub path: String,
    pub total_score: u32,
    pub violations: Vec<RuleViolation>,
}

/// Tier rating for a file's stink score.
pub struct Tier {
    pub emoji: &'static str,
    pub name: &'static str,
    pub message: &'static str,
    pub color: &'static str,
}

/// Get the tier for a given stink score.
pub fn tier_for_score(score: u32) -> Tier {
    match score {
        0..=30 => Tier {
            emoji: "👶",
            name: "Fresh Baby",
            message: "Squeaky clean.",
            color: GREEN,
        },
        31..=70 => Tier {
            emoji: "💪",
            name: "Loaded",
            message: "A little dirty, but sometimes a little dirt in the diaper is worth it.",
            color: YELLOW,
        },
        71..=99 => Tier {
            emoji: "🧨",
            name: "Blowout Warning",
            message: "Don't leave this too long or you'll get a rash",
            color: RED,
        },
        _ => Tier {
            emoji: "💩",
            name: "SOILED",
            message: "SOILED. Must change.",
            color: BRIGHT_RED,
        },
    }
}

/// Check a single file against all rules.
pub fn check_file(path: &str, cache: &mut AstCache) -> Result<FileResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {path}: {e}"))?;

    let tree = rules::parse_js(&source)
        .ok_or_else(|| format!("failed to parse {path}"))?;

    let file_path = Path::new(path);

    // Pre-seed cache with this file's already-parsed tree
    if let Ok(abs) = fs::canonicalize(file_path) {
        cache.insert(abs, source.clone(), tree.clone());
    }

    let rules = rules::all_rules();

    let mut violations = Vec::new();
    for rule in &rules {
        let mut rule_violations = rule.check(&source, file_path, &tree, cache);
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

/// Check multiple files and print results.
pub fn check_files(paths: &[String]) -> Result<(), String> {
    let js_paths: Vec<&String> = paths.iter()
        .filter(|p| p.ends_with(".js"))
        .collect();

    if js_paths.is_empty() {
        println!("{DIM}no JavaScript files to check{RESET}");
        return Ok(());
    }

    let mut cache = AstCache::new();
    let mut any_smells = false;

    for path in &js_paths {
        let result = check_file(path, &mut cache)?;

        if result.total_score > 0 {
            any_smells = true;
            let tier = tier_for_score(result.total_score);
            println!(
                "{} {BOLD}{}{RESET}  {}stink: {}  {}{RESET}",
                tier.emoji, result.path, tier.color, result.total_score, tier.name
            );
            for violation in &result.violations {
                let doc_link = hyperlink(&violation.doc_url, "docs");
                println!("  {YELLOW}+{}{RESET}  {DIM}{}{RESET}  {}  {DIM}{doc_link}{RESET}", violation.score, violation.rule_name, violation.message);
            }
            println!("  {DIM}{}{RESET}", tier.message);
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
        let mut cache = AstCache::new();
        let result = check_file(file.path().to_str().unwrap(), &mut cache).unwrap();
        assert_eq!(result.total_score, 0);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_check_file_long_file() {
        let source = make_js_source(300);
        let file = create_temp_js_file(&source);
        let mut cache = AstCache::new();
        let result = check_file(file.path().to_str().unwrap(), &mut cache).unwrap();
        assert_eq!(result.total_score, 20);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_check_file_nonexistent() {
        let mut cache = AstCache::new();
        let result = check_file("/tmp/nonexistent_diaper_test.js", &mut cache);
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
        assert!(link.contains("\x1b]8;;"));
    }

    // --- Tier tests ---

    #[test]
    fn test_tier_zero() {
        let tier = tier_for_score(0);
        assert_eq!(tier.emoji, "👶");
        assert_eq!(tier.name, "Fresh Baby");
    }

    #[test]
    fn test_tier_squeaky_clean_boundary() {
        let tier = tier_for_score(30);
        assert_eq!(tier.emoji, "👶");
    }

    #[test]
    fn test_tier_loaded_low() {
        let tier = tier_for_score(31);
        assert_eq!(tier.emoji, "💪");
        assert_eq!(tier.name, "Loaded");
    }

    #[test]
    fn test_tier_loaded_high() {
        let tier = tier_for_score(70);
        assert_eq!(tier.emoji, "💪");
    }

    #[test]
    fn test_tier_blowout_low() {
        let tier = tier_for_score(71);
        assert_eq!(tier.emoji, "🧨");
        assert_eq!(tier.name, "Blowout Warning");
    }

    #[test]
    fn test_tier_blowout_high() {
        let tier = tier_for_score(99);
        assert_eq!(tier.emoji, "🧨");
    }

    #[test]
    fn test_tier_soiled_exact() {
        let tier = tier_for_score(100);
        assert_eq!(tier.emoji, "💩");
        assert_eq!(tier.name, "SOILED");
    }

    #[test]
    fn test_tier_soiled_high() {
        let tier = tier_for_score(500);
        assert_eq!(tier.emoji, "💩");
    }

    #[test]
    fn test_tier_colors() {
        assert_eq!(tier_for_score(0).color, GREEN);
        assert_eq!(tier_for_score(50).color, YELLOW);
        assert_eq!(tier_for_score(80).color, RED);
        assert_eq!(tier_for_score(100).color, BRIGHT_RED);
    }
}
