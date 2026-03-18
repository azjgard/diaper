use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "diaper.yml";

/// Configuration loaded from diaper.yml.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// Override stink scores per rule. Key is the rule name (e.g. "async-await").
    #[serde(default)]
    pub rules: HashMap<String, u32>,

    /// Override tier level minimums. Key is the tier name (lowercase).
    #[serde(default)]
    pub levels: HashMap<String, u32>,
}

/// Default tier thresholds.
pub const DEFAULT_DAMP_MIN: u32 = 0;
pub const DEFAULT_WET_MIN: u32 = 31;
pub const DEFAULT_SOILED_MIN: u32 = 71;
pub const DEFAULT_BLOWOUT_MIN: u32 = 100;

/// Default rule scores.
pub const DEFAULT_ASYNC_AWAIT: u32 = 100;
pub const DEFAULT_FILE_TOO_LONG: u32 = 10;
pub const DEFAULT_NON_DEFAULT_EXPORT: u32 = 50;
pub const DEFAULT_TERNARY_SINGLE: u32 = 10;
pub const DEFAULT_TERNARY_NESTED: u32 = 60;
pub const DEFAULT_UPWARD_RELATIVE_IMPORT: u32 = 100;
pub const DEFAULT_PIPE_PROPERTY_INIT: u32 = 100;

impl Config {
    /// Load config from diaper.yml in the current directory.
    /// Returns default config if the file doesn't exist.
    pub fn load() -> Result<Config, String> {
        let path = Path::new(CONFIG_FILE);
        if !path.exists() {
            return Ok(Config::default());
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {CONFIG_FILE}: {e}"))?;

        let config: Config = serde_yaml::from_str(&contents)
            .map_err(|e| format!("failed to parse {CONFIG_FILE}: {e}"))?;

        Ok(config)
    }

    /// Get the score for a rule, falling back to the provided default.
    pub fn rule_score(&self, rule_name: &str, default: u32) -> u32 {
        self.rules.get(rule_name).copied().unwrap_or(default)
    }

    /// Get the tier level minimums, falling back to defaults.
    pub fn level_min(&self, level_name: &str, default: u32) -> u32 {
        self.levels.get(level_name).copied().unwrap_or(default)
    }
}

/// Generate a default diaper.yml file.
pub fn generate_default_config() -> String {
    format!(
        r#"# diaper configuration
# Override stink scores per rule and tier level thresholds.
# Remove or comment out any line to use the default value.

rules:
  async-await: {DEFAULT_ASYNC_AWAIT}
  file-too-long: {DEFAULT_FILE_TOO_LONG}
  non-default-export: {DEFAULT_NON_DEFAULT_EXPORT}
  ternary-single: {DEFAULT_TERNARY_SINGLE}
  ternary-nested: {DEFAULT_TERNARY_NESTED}
  upward-relative-import: {DEFAULT_UPWARD_RELATIVE_IMPORT}
  pipe-property-init: {DEFAULT_PIPE_PROPERTY_INIT}

levels:
  damp: {DEFAULT_DAMP_MIN}
  wet: {DEFAULT_WET_MIN}
  soiled: {DEFAULT_SOILED_MIN}
  blowout: {DEFAULT_BLOWOUT_MIN}
"#
    )
}

/// Write the default config file to the current directory.
pub fn init_config() -> Result<(), String> {
    let path = Path::new(CONFIG_FILE);
    if path.exists() {
        return Err(format!("{CONFIG_FILE} already exists"));
    }

    let content = generate_default_config();
    fs::write(path, content)
        .map_err(|e| format!("failed to write {CONFIG_FILE}: {e}"))?;

    println!("created {CONFIG_FILE}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.rules.is_empty());
        assert!(config.levels.is_empty());
    }

    #[test]
    fn test_rule_score_with_override() {
        let mut config = Config::default();
        config.rules.insert("async-await".to_string(), 200);
        assert_eq!(config.rule_score("async-await", 100), 200);
    }

    #[test]
    fn test_rule_score_fallback() {
        let config = Config::default();
        assert_eq!(config.rule_score("async-await", 100), 100);
    }

    #[test]
    fn test_level_min_with_override() {
        let mut config = Config::default();
        config.levels.insert("soiled".to_string(), 200);
        assert_eq!(config.level_min("soiled", 100), 200);
    }

    #[test]
    fn test_level_min_fallback() {
        let config = Config::default();
        assert_eq!(config.level_min("soiled", 100), 100);
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
rules:
  async-await: 50
  file-too-long: 5
levels:
  soiled: 200
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rules["async-await"], 50);
        assert_eq!(config.rules["file-too-long"], 5);
        assert_eq!(config.levels["soiled"], 200);
    }

    #[test]
    fn test_parse_empty_yaml() {
        let yaml = "";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.rules.is_empty());
        assert!(config.levels.is_empty());
    }

    #[test]
    fn test_parse_partial_yaml() {
        let yaml = "rules:\n  async-await: 25\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rules["async-await"], 25);
        assert!(config.levels.is_empty());
    }

    #[test]
    fn test_generate_default_config_contains_all_rules() {
        let content = generate_default_config();
        assert!(content.contains("async-await:"));
        assert!(content.contains("file-too-long:"));
        assert!(content.contains("non-default-export:"));
        assert!(content.contains("ternary-single:"));
        assert!(content.contains("ternary-nested:"));
        assert!(content.contains("upward-relative-import:"));
        assert!(content.contains("pipe-property-init:"));
    }

    #[test]
    fn test_generate_default_config_contains_all_levels() {
        let content = generate_default_config();
        assert!(content.contains("damp:"));
        assert!(content.contains("wet:"));
        assert!(content.contains("soiled:"));
        assert!(content.contains("blowout:"));
    }

    #[test]
    fn test_load_missing_file() {
        // When run from a dir without diaper.yml, should return defaults
        let config = Config::load().unwrap();
        assert!(config.rules.is_empty());
    }
}
