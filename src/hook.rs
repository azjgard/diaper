use std::fs;
use std::path::PathBuf;

const HOOK_SCRIPT_NAME: &str = "diaper-check.sh";
const PROJECT_DIR: &str = "/Users/jordin/projects/core/api-gateway";

fn hooks_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".claude").join("hooks")
}

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".claude").join("settings.json")
}

fn generate_hook_script() -> String {
    // Find the diaper binary path
    let diaper_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "diaper".to_string());

    let mut script = String::new();
    script.push_str("#!/bin/bash\n");
    script.push('\n');
    script.push_str("# Only run in the api-gateway project\n");
    script.push_str(&format!("[[ \"$PWD\" != \"{PROJECT_DIR}\" ]] && exit 0\n"));
    script.push('\n');
    script.push_str("# Allow Claude to accept violations and skip this check once\n");
    script.push_str("[[ -f /tmp/diaper-check-accept ]] && rm -f /tmp/diaper-check-accept && exit 0\n");
    script.push('\n');
    script.push_str("# Only run if there are unstaged changes to JavaScript files\n");
    script.push_str("git diff --name-only -- '*.js' | grep -q . || exit 0\n");
    script.push('\n');
    script.push_str("# Run diaper check with JSON output\n");
    script.push_str(&format!("OUTPUT=$({diaper_bin} check --json 2>&1)\n"));
    script.push('\n');
    script.push_str("# Exit cleanly if there are no violations\n");
    script.push_str("echo \"$OUTPUT\" | jq -e 'length > 0' > /dev/null 2>&1 || exit 0\n");
    script.push('\n');
    script.push_str("# Block Claude from stopping and inject violations into context\n");
    script.push_str("echo \"{\\\"decision\\\":\\\"block\\\",\\\"reason\\\":$(echo \"diaper check found violations. Review and fix them, or if acceptable, run: touch /tmp/diaper-check-accept\\n\\n$OUTPUT\" | jq -Rs .)}\"\n");
    script
}

/// Check if the Stop hook for diaper is already present in settings.json.
fn hook_already_installed(settings: &serde_json::Value) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(|s| s.as_array())
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .is_some_and(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|cmd| cmd.contains(HOOK_SCRIPT_NAME))
                        })
                    })
            })
        })
}

/// Add the diaper stop hook entry to a settings JSON value.
/// Returns true if the hook was added, false if it was already present.
fn add_hook_to_settings(settings: &mut serde_json::Value, script_path: &str) -> Result<bool, String> {
    if hook_already_installed(settings) {
        return Ok(false);
    }

    let hook_entry = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": script_path,
            "statusMessage": "Running diaper check..."
        }]
    });

    let hooks = settings
        .as_object_mut()
        .ok_or("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let stop = hooks
        .as_object_mut()
        .ok_or("hooks is not an object")?
        .entry("Stop")
        .or_insert_with(|| serde_json::json!([]));

    stop.as_array_mut()
        .ok_or("hooks.Stop is not an array")?
        .push(hook_entry);

    Ok(true)
}

/// Install the Claude Code Stop hook for diaper.
pub fn install_hook() -> Result<(), String> {
    let script_path = hooks_dir().join(HOOK_SCRIPT_NAME);
    let settings_file = settings_path();

    install_hook_to(&script_path, &settings_file)
}

/// Core install logic, testable with arbitrary paths.
fn install_hook_to(script_path: &std::path::Path, settings_file: &std::path::Path) -> Result<(), String> {
    // 1. Write the hook script
    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create hooks dir: {e}"))?;
    }

    fs::write(script_path, generate_hook_script())
        .map_err(|e| format!("failed to write hook script: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(script_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to chmod hook script: {e}"))?;
    }

    println!("wrote {}", script_path.display());

    // 2. Update settings.json
    let mut settings: serde_json::Value = if settings_file.exists() {
        let contents = fs::read_to_string(settings_file)
            .map_err(|e| format!("failed to read settings.json: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse settings.json: {e}"))?
    } else {
        serde_json::json!({})
    };

    let script_path_str = script_path.to_string_lossy().to_string();
    let added = add_hook_to_settings(&mut settings, &script_path_str)?;

    if !added {
        println!("stop hook already installed in settings.json, skipping");
        return Ok(());
    }

    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize settings.json: {e}"))?;

    fs::write(settings_file, output + "\n")
        .map_err(|e| format!("failed to write settings.json: {e}"))?;

    println!("added stop hook to {}", settings_file.display());
    println!();
    println!("diaper will now check for violations when Claude stops in {PROJECT_DIR}");
    println!();
    println!("tip: run Claude in bypass permissions mode so it can touch /tmp/diaper-check-accept");
    println!("     to accept violations without getting prompted:");
    println!("       claude --dangerously-skip-permissions");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- generate_hook_script tests ---

    #[test]
    fn test_generate_hook_script_contains_project_dir() {
        let script = generate_hook_script();
        assert!(script.contains(PROJECT_DIR));
    }

    #[test]
    fn test_generate_hook_script_contains_accept_hatch() {
        let script = generate_hook_script();
        assert!(script.contains("/tmp/diaper-check-accept"));
    }

    #[test]
    fn test_generate_hook_script_contains_json_flag() {
        let script = generate_hook_script();
        assert!(script.contains("--json"));
    }

    #[test]
    fn test_generate_hook_script_contains_block_decision() {
        let script = generate_hook_script();
        assert!(script.contains(r#"\"decision\":\"block\""#));
    }

    #[test]
    fn test_generate_hook_script_starts_with_shebang() {
        let script = generate_hook_script();
        assert!(script.starts_with("#!/bin/bash\n"));
    }

    #[test]
    fn test_generate_hook_script_checks_js_files_only() {
        let script = generate_hook_script();
        assert!(script.contains("'*.js'"));
    }

    // --- hook_already_installed tests ---

    #[test]
    fn test_hook_already_installed_empty() {
        let settings = serde_json::json!({});
        assert!(!hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_no_stop() {
        let settings = serde_json::json!({ "hooks": {} });
        assert!(!hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_empty_stop_array() {
        let settings = serde_json::json!({ "hooks": { "Stop": [] } });
        assert!(!hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_other_stop_hook() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo done"
                    }]
                }]
            }
        });
        assert!(!hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_true() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/Users/jordin/.claude/hooks/diaper-check.sh"
                    }]
                }]
            }
        });
        assert!(hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_among_others() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [{
                            "type": "command",
                            "command": "echo other"
                        }]
                    },
                    {
                        "hooks": [{
                            "type": "command",
                            "command": "/home/user/.claude/hooks/diaper-check.sh"
                        }]
                    }
                ]
            }
        });
        assert!(hook_already_installed(&settings));
    }

    #[test]
    fn test_hook_already_installed_different_path_same_filename() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/some/other/path/diaper-check.sh"
                    }]
                }]
            }
        });
        assert!(hook_already_installed(&settings));
    }

    // --- add_hook_to_settings tests ---

    #[test]
    fn test_add_hook_to_empty_settings() {
        let mut settings = serde_json::json!({});
        let added = add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();
        assert!(added);

        let stop = &settings["hooks"]["Stop"];
        assert_eq!(stop.as_array().unwrap().len(), 1);
        assert_eq!(
            stop[0]["hooks"][0]["command"].as_str().unwrap(),
            "/path/to/diaper-check.sh"
        );
        assert_eq!(
            stop[0]["hooks"][0]["statusMessage"].as_str().unwrap(),
            "Running diaper check..."
        );
    }

    #[test]
    fn test_add_hook_preserves_existing_hooks() {
        let mut settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "echo pre"
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo existing-stop"
                    }]
                }]
            }
        });
        let added = add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();
        assert!(added);

        // PreToolUse still there
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap(),
            "echo pre"
        );

        // Stop now has 2 entries
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing-stop");
        assert_eq!(stop[1]["hooks"][0]["command"].as_str().unwrap(), "/path/to/diaper-check.sh");
    }

    #[test]
    fn test_add_hook_preserves_non_hook_settings() {
        let mut settings = serde_json::json!({
            "permissions": {
                "allow": ["Bash(git *)"]
            },
            "model": "opus",
            "hooks": {}
        });
        let added = add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();
        assert!(added);

        // Other settings untouched
        assert_eq!(settings["permissions"]["allow"][0].as_str().unwrap(), "Bash(git *)");
        assert_eq!(settings["model"].as_str().unwrap(), "opus");
    }

    #[test]
    fn test_add_hook_idempotent() {
        let mut settings = serde_json::json!({});
        add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();

        let added = add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();
        assert!(!added);

        // Still only 1 entry
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_add_hook_preserves_post_tool_use_hooks() {
        let mut settings = serde_json::json!({
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Write|Edit",
                        "hooks": [{
                            "type": "command",
                            "command": "prettier --write"
                        }]
                    }
                ]
            }
        });
        add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();

        // PostToolUse untouched
        let post = &settings["hooks"]["PostToolUse"];
        assert_eq!(post.as_array().unwrap().len(), 1);
        assert_eq!(post[0]["matcher"].as_str().unwrap(), "Write|Edit");
    }

    #[test]
    fn test_add_hook_preserves_session_start_hooks() {
        let mut settings = serde_json::json!({
            "hooks": {
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo hello"
                    }]
                }]
            }
        });
        add_hook_to_settings(&mut settings, "/path/to/diaper-check.sh").unwrap();

        assert_eq!(settings["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"].as_str().unwrap(),
            "echo hello"
        );
    }

    // --- install_hook_to filesystem tests ---

    #[test]
    fn test_install_creates_hook_script() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        assert!(script_path.exists());
        let contents = fs::read_to_string(&script_path).unwrap();
        assert!(contents.starts_with("#!/bin/bash"));
        assert!(contents.contains(PROJECT_DIR));
    }

    #[test]
    fn test_install_creates_settings_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        assert!(settings_path.exists());
        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_install_preserves_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        let existing = serde_json::json!({
            "permissions": {
                "allow": ["Bash(git *)"]
            },
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "echo pre"
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo existing"
                    }]
                }]
            },
            "model": "opus"
        });
        fs::write(&settings_path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_hook_to(&script_path, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();

        // permissions preserved
        assert_eq!(settings["permissions"]["allow"][0].as_str().unwrap(), "Bash(git *)");
        // model preserved
        assert_eq!(settings["model"].as_str().unwrap(), "opus");
        // PreToolUse preserved
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        // existing Stop hook preserved + new one added
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing");
        assert!(stop[1]["hooks"][0]["command"].as_str().unwrap().contains("diaper-check.sh"));
    }

    #[test]
    fn test_install_idempotent_does_not_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();
        install_hook_to(&script_path, &settings_path).unwrap();
        install_hook_to(&script_path, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();

        // Still only 1 Stop hook entry
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_install_does_not_touch_other_files_in_hooks_dir() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Create some existing hook files
        fs::write(hooks_dir.join("on-edit.sh"), "#!/bin/bash\necho edit").unwrap();
        fs::write(hooks_dir.join("safety-block.js"), "console.log('safe')").unwrap();

        let script_path = hooks_dir.join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        // Other files untouched
        assert_eq!(fs::read_to_string(hooks_dir.join("on-edit.sh")).unwrap(), "#!/bin/bash\necho edit");
        assert_eq!(fs::read_to_string(hooks_dir.join("safety-block.js")).unwrap(), "console.log('safe')");
    }

    #[test]
    fn test_install_script_is_executable() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::metadata(&script_path).unwrap().permissions();
            assert!(perms.mode() & 0o111 != 0, "hook script should be executable");
        }
    }

    #[test]
    fn test_install_settings_is_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let result: Result<serde_json::Value, _> = serde_json::from_str(&contents);
        assert!(result.is_ok(), "settings.json should be valid JSON");
    }

    #[test]
    fn test_install_settings_is_pretty_printed() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        install_hook_to(&script_path, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        assert!(contents.contains('\n'), "settings.json should be pretty-printed");
        assert!(contents.contains("  "), "settings.json should have indentation");
    }

    #[test]
    fn test_install_preserves_complex_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hooks").join("diaper-check.sh");
        let settings_path = dir.path().join("settings.json");

        // Simulate a realistic settings.json like the one in ~/.claude/settings.json
        let existing = serde_json::json!({
            "permissions": {
                "allow": [
                    "Bash(gh api)",
                    "Bash(git *)",
                    "Bash(ls *)"
                ]
            },
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash|Write|Edit",
                    "hooks": [{
                        "type": "command",
                        "command": "node /Users/jordin/.claude/hooks/safety-block.js",
                        "timeout": 5
                    }]
                }],
                "PostToolUse": [
                    {
                        "matcher": "Write|Edit",
                        "hooks": [{
                            "type": "command",
                            "command": "/Users/jordin/.claude/hooks/on-edit.sh"
                        }]
                    },
                    {
                        "hooks": [{
                            "type": "command",
                            "command": "node \"/Users/jordin/.claude/hooks/gsd-context-monitor.js\""
                        }]
                    }
                ],
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": "node \"/Users/jordin/.claude/hooks/gsd-check-update.js\""
                    }]
                }]
            },
            "statusLine": {
                "type": "command",
                "command": "node \"/Users/jordin/.claude/hooks/gsd-statusline.js\""
            },
            "enabledPlugins": {
                "lua-lsp@claude-plugins-official": true,
                "rust-analyzer-lsp@claude-plugins-official": true
            },
            "alwaysThinkingEnabled": false
        });
        fs::write(&settings_path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_hook_to(&script_path, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();

        // Verify everything is preserved
        assert_eq!(settings["permissions"]["allow"].as_array().unwrap().len(), 3);
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"].as_str().unwrap(), "Bash|Write|Edit");
        assert_eq!(settings["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(settings["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(settings["statusLine"]["type"].as_str().unwrap(), "command");
        assert_eq!(settings["enabledPlugins"]["lua-lsp@claude-plugins-official"].as_bool().unwrap(), true);
        assert_eq!(settings["alwaysThinkingEnabled"].as_bool().unwrap(), false);

        // New Stop hook added
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }
}
