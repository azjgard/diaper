use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::config::{self, Config};
use crate::rules::{self, AstCache, RuleViolation};

// ANSI color codes
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const ORANGE: &str = "\x1b[38;5;208m";
const RED: &str = "\x1b[31m";
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

/// Get the tier for a given stink score, using config for level thresholds.
pub fn tier_for_score(score: u32, config: &Config) -> Tier {
    let blowout_min = config.level_min("blowout", config::DEFAULT_BLOWOUT_MIN);
    let soiled_min = config.level_min("soiled", config::DEFAULT_SOILED_MIN);
    let wet_min = config.level_min("wet", config::DEFAULT_WET_MIN);

    if score >= blowout_min {
        Tier {
            emoji: "💩",
            name: "BLOWOUT",
            message: "BLOWOUT. Must change.",
            color: RED,
        }
    } else if score >= soiled_min {
        Tier {
            emoji: "🧨",
            name: "Soiled",
            message: "Don't leave this too long or you'll get a rash",
            color: ORANGE,
        }
    } else if score >= wet_min {
        Tier {
            emoji: "💪",
            name: "Wet",
            message: "A little dirty, but sometimes a little dirt in the diaper is worth it.",
            color: YELLOW,
        }
    } else {
        Tier {
            emoji: "👶",
            name: "Damp",
            message: "Barely noticeable.",
            color: GREEN,
        }
    }
}

/// Check a single file against all rules.
pub fn check_file(path: &str, cache: &mut AstCache, config: &Config) -> Result<FileResult, String> {
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
        let mut rule_violations = rule.check(&source, file_path, &tree, cache, config);
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
pub fn check_files(paths: &[String], config: &Config) -> Result<(), String> {
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
        let result = check_file(path, &mut cache, config)?;

        if result.total_score > 0 {
            any_smells = true;
            let tier = tier_for_score(result.total_score, config);
            println!(
                "{BOLD}{}{RESET}  {}{} {} ({}){RESET}",
                result.path, tier.color, tier.name, tier.emoji, result.total_score
            );
            for violation in &result.violations {
                let doc_link = hyperlink(&violation.doc_url, "docs");
                println!("  {YELLOW}+{}{RESET}  {DIM}{}{RESET}  {}  {DIM}{doc_link}{RESET}", violation.score, violation.rule_name, violation.code_sample);
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

#[derive(Serialize)]
struct JsonFileResult {
    path: String,
    #[serde(rename = "stinkScore")]
    stink_score: u32,
    violations: Vec<JsonViolation>,
}

#[derive(Serialize)]
struct JsonViolation {
    rule: String,
    #[serde(rename = "stinkScore")]
    stink_score: u32,
    #[serde(rename = "codeSample")]
    code_sample: String,
    reference: String,
}

/// Check multiple files and print results as JSON.
pub fn check_files_json(paths: &[String], config: &Config) -> Result<(), String> {
    let js_paths: Vec<&String> = paths.iter()
        .filter(|p| p.ends_with(".js"))
        .collect();

    let mut cache = AstCache::new();
    let mut results: Vec<JsonFileResult> = Vec::new();

    for path in &js_paths {
        let result = check_file(path, &mut cache, config)?;

        if result.total_score > 0 {
            results.push(JsonFileResult {
                path: result.path,
                stink_score: result.total_score,
                violations: result.violations.into_iter().map(|v| JsonViolation {
                    rule: v.rule_name,
                    stink_score: v.score,
                    code_sample: v.code_sample,
                    reference: v.doc_url,
                }).collect(),
            });
        }
    }

    let json = serde_json::to_string_pretty(&results)
        .map_err(|e| format!("failed to serialize JSON: {e}"))?;
    println!("{json}");

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

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_check_file_short_file() {
        let file = create_temp_js_file("const x = 1;\n");
        let mut cache = AstCache::new();
        let config = default_config();
        let result = check_file(file.path().to_str().unwrap(), &mut cache, &config).unwrap();
        assert_eq!(result.total_score, 0);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_check_file_long_file() {
        let source = make_js_source(300);
        let file = create_temp_js_file(&source);
        let mut cache = AstCache::new();
        let config = default_config();
        let result = check_file(file.path().to_str().unwrap(), &mut cache, &config).unwrap();
        assert_eq!(result.total_score, 20);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_check_file_nonexistent() {
        let mut cache = AstCache::new();
        let config = default_config();
        let result = check_file("/tmp/nonexistent_diaper_test.js", &mut cache, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_files_filters_non_js() {
        let files = vec!["foo.rs".to_string(), "bar.py".to_string()];
        let config = default_config();
        let result = check_files(&files, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_files_with_js_file() {
        let source = make_js_source(10);
        let file = create_temp_js_file(&source);
        let path = file.path().to_str().unwrap().to_string();
        let config = default_config();
        let result = check_files(&[path], &config);
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
    fn test_tier_damp() {
        let c = default_config();
        let tier = tier_for_score(0, &c);
        assert_eq!(tier.emoji, "👶");
        assert_eq!(tier.name, "Damp");
    }

    #[test]
    fn test_tier_damp_boundary() {
        let c = default_config();
        let tier = tier_for_score(30, &c);
        assert_eq!(tier.emoji, "👶");
    }

    #[test]
    fn test_tier_wet_low() {
        let c = default_config();
        let tier = tier_for_score(31, &c);
        assert_eq!(tier.emoji, "💪");
        assert_eq!(tier.name, "Wet");
    }

    #[test]
    fn test_tier_wet_high() {
        let c = default_config();
        let tier = tier_for_score(70, &c);
        assert_eq!(tier.emoji, "💪");
    }

    #[test]
    fn test_tier_soiled_low() {
        let c = default_config();
        let tier = tier_for_score(71, &c);
        assert_eq!(tier.emoji, "🧨");
        assert_eq!(tier.name, "Soiled");
    }

    #[test]
    fn test_tier_soiled_high() {
        let c = default_config();
        let tier = tier_for_score(99, &c);
        assert_eq!(tier.emoji, "🧨");
    }

    #[test]
    fn test_tier_blowout_exact() {
        let c = default_config();
        let tier = tier_for_score(100, &c);
        assert_eq!(tier.emoji, "💩");
        assert_eq!(tier.name, "BLOWOUT");
    }

    #[test]
    fn test_tier_blowout_high() {
        let c = default_config();
        let tier = tier_for_score(500, &c);
        assert_eq!(tier.emoji, "💩");
    }

    #[test]
    fn test_tier_colors() {
        let c = default_config();
        assert_eq!(tier_for_score(0, &c).color, GREEN);
        assert_eq!(tier_for_score(50, &c).color, YELLOW);
        assert_eq!(tier_for_score(80, &c).color, ORANGE);
        assert_eq!(tier_for_score(100, &c).color, RED);
    }
}
