use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "diaper.yml";

/// Per-rule configuration. Can be either a bare score number or an object
/// with score and optional docs path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfig {
    /// Just a score: `async-await: 100`
    Score(u32),
    /// Score with optional docs: `async-await: { score: 100, docs: "./docs/async-await.md" }`
    Full {
        score: u32,
        #[serde(default)]
        docs: Option<String>,
    },
}

impl RuleConfig {
    pub fn score(&self) -> u32 {
        match self {
            RuleConfig::Score(s) => *s,
            RuleConfig::Full { score, .. } => *score,
        }
    }

    pub fn docs(&self) -> Option<&str> {
        match self {
            RuleConfig::Score(_) => None,
            RuleConfig::Full { docs, .. } => docs.as_deref(),
        }
    }
}

/// Configuration loaded from diaper.yml.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// Override stink scores and docs per rule. Key is the rule name (e.g. "async-await").
    #[serde(default)]
    pub rules: HashMap<String, RuleConfig>,

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
pub const DEFAULT_ASYNC_PROMISE_RETURN: u32 = 15;
pub const DEFAULT_CTX_DESTRUCTURE: u32 = 10;
pub const DEFAULT_FILE_TOO_LONG: u32 = 10;
pub const DEFAULT_GRAPHQL_TYPE_EXPORT: u32 = 100;
pub const DEFAULT_NON_DEFAULT_EXPORT: u32 = 50;
pub const DEFAULT_NON_IDEMPOTENT_MIGRATION: u32 = 50;
pub const DEFAULT_TERNARY_SINGLE: u32 = 10;
pub const DEFAULT_TERNARY_NESTED: u32 = 60;
pub const DEFAULT_UNSORTED_STRING_ARRAY: u32 = 5;
pub const DEFAULT_SQL_TABLE_ALIAS: u32 = 100;
pub const DEFAULT_UPWARD_RELATIVE_IMPORT: u32 = 100;
pub const DEFAULT_PIPE_PROPERTY_INIT: u32 = 100;
pub const DEFAULT_REDUCE_PARAM_NAME: u32 = 70;
pub const DEFAULT_REQUIRE_QUERY_ATTRIBUTES: u32 = 10;
pub const DEFAULT_SHORT_ITER_PARAM: u32 = 15;

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
        self.rules.get(rule_name).map(|r| r.score()).unwrap_or(default)
    }

    /// Get the docs path for a rule, if configured.
    /// Returns an absolute path resolved relative to the config file's directory.
    pub fn rule_docs(&self, rule_name: &str) -> Option<String> {
        let docs_path = self.rules.get(rule_name)?.docs()?;
        // Resolve relative to CWD (where diaper.yml lives)
        let path = Path::new(docs_path);
        if path.is_absolute() {
            Some(docs_path.to_string())
        } else {
            Some(path.to_string_lossy().to_string())
        }
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
#
# Rules can be a bare score or an object with score and docs path:
#   async-await: 100
#   async-await:
#     score: 100
#     docs: ./docs/rules/async-await.md

rules:
  async-await: {DEFAULT_ASYNC_AWAIT}
  async-promise-return: {DEFAULT_ASYNC_PROMISE_RETURN}
  ctx-destructure: {DEFAULT_CTX_DESTRUCTURE}
  file-too-long: {DEFAULT_FILE_TOO_LONG}
  graphql-type-export: {DEFAULT_GRAPHQL_TYPE_EXPORT}
  non-default-export: {DEFAULT_NON_DEFAULT_EXPORT}
  non-idempotent-migration: {DEFAULT_NON_IDEMPOTENT_MIGRATION}
  ternary-single: {DEFAULT_TERNARY_SINGLE}
  ternary-nested: {DEFAULT_TERNARY_NESTED}
  unsorted-string-array: {DEFAULT_UNSORTED_STRING_ARRAY}
  upward-relative-import: {DEFAULT_UPWARD_RELATIVE_IMPORT}
  pipe-property-init: {DEFAULT_PIPE_PROPERTY_INIT}
  reduce-param-name: {DEFAULT_REDUCE_PARAM_NAME}
  require-query-attributes: {DEFAULT_REQUIRE_QUERY_ATTRIBUTES}
  short-iter-param: {DEFAULT_SHORT_ITER_PARAM}
  sql-table-alias: {DEFAULT_SQL_TABLE_ALIAS}

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
        config.rules.insert("async-await".to_string(), RuleConfig::Score(200));
        assert_eq!(config.rule_score("async-await", 100), 200);
    }

    #[test]
    fn test_rule_score_fallback() {
        let config = Config::default();
        assert_eq!(config.rule_score("async-await", 100), 100);
    }

    #[test]
    fn test_rule_score_full_config() {
        let mut config = Config::default();
        config.rules.insert("async-await".to_string(), RuleConfig::Full {
            score: 75,
            docs: Some("./docs/async.md".to_string()),
        });
        assert_eq!(config.rule_score("async-await", 100), 75);
    }

    #[test]
    fn test_rule_docs_none_by_default() {
        let config = Config::default();
        assert!(config.rule_docs("async-await").is_none());
    }

    #[test]
    fn test_rule_docs_none_for_bare_score() {
        let mut config = Config::default();
        config.rules.insert("async-await".to_string(), RuleConfig::Score(100));
        assert!(config.rule_docs("async-await").is_none());
    }

    #[test]
    fn test_rule_docs_with_path() {
        let mut config = Config::default();
        config.rules.insert("async-await".to_string(), RuleConfig::Full {
            score: 100,
            docs: Some("./docs/async-await.md".to_string()),
        });
        assert_eq!(config.rule_docs("async-await").unwrap(), "./docs/async-await.md");
    }

    #[test]
    fn test_rule_docs_full_without_docs() {
        let mut config = Config::default();
        config.rules.insert("async-await".to_string(), RuleConfig::Full {
            score: 100,
            docs: None,
        });
        assert!(config.rule_docs("async-await").is_none());
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
    fn test_parse_yaml_bare_scores() {
        let yaml = r#"
rules:
  async-await: 50
  file-too-long: 5
levels:
  soiled: 200
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rule_score("async-await", 100), 50);
        assert_eq!(config.rule_score("file-too-long", 10), 5);
        assert_eq!(config.levels["soiled"], 200);
    }

    #[test]
    fn test_parse_yaml_full_config() {
        let yaml = r#"
rules:
  async-await:
    score: 75
    docs: ./docs/async-await.md
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rule_score("async-await", 100), 75);
        assert_eq!(config.rule_docs("async-await").unwrap(), "./docs/async-await.md");
    }

    #[test]
    fn test_parse_yaml_mixed() {
        let yaml = r#"
rules:
  async-await: 50
  file-too-long:
    score: 5
    docs: ./docs/file-too-long.md
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rule_score("async-await", 100), 50);
        assert!(config.rule_docs("async-await").is_none());
        assert_eq!(config.rule_score("file-too-long", 10), 5);
        assert_eq!(config.rule_docs("file-too-long").unwrap(), "./docs/file-too-long.md");
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
        assert_eq!(config.rule_score("async-await", 100), 25);
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
