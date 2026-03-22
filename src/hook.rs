use std::fs;
use std::path::PathBuf;

const STOP_HOOK_SCRIPT: &str = "diaper-check.sh";
const PRE_EDIT_HOOK_SCRIPT: &str = "diaper-pre-edit.sh";

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
    script.push_str("# Only run in api-gateway projects\n");
    script.push_str("[[ \"$PWD\" != *\"api-gateway\"* ]] && exit 0\n");
    script.push('\n');
    script.push_str("# Only run if there are unstaged changes to JavaScript files\n");
    script.push_str("git diff --name-only -- '*.js' | grep -q . || exit 0\n");
    script.push('\n');
    script.push_str("# Run diaper check with JSON output\n");
    script.push_str(&format!("OUTPUT=$({diaper_bin} check --json 2>&1)\n"));
    script.push_str("EXIT_CODE=$?\n");
    script.push('\n');
    script.push_str("# Exit cleanly if there are no violations\n");
    script.push_str("echo \"$OUTPUT\" | jq -e 'length > 0' > /dev/null 2>&1 || exit 0\n");
    script.push('\n');
    script.push_str("# Blowouts (exit code 1) must be fixed — no escape hatch\n");
    script.push_str("if [ \"$EXIT_CODE\" -ne 0 ]; then\n");
    script.push_str("  echo \"{\\\"decision\\\":\\\"block\\\",\\\"reason\\\":$(echo \"diaper check found BLOWOUT violations that must be fixed.\\n\\n$OUTPUT\" | jq -Rs .)}\"\n");
    script.push_str("  exit 0\n");
    script.push_str("fi\n");
    script.push('\n');
    script.push_str("# Non-blowout violations can be accepted via sentinel file\n");
    script.push_str("[[ -f /tmp/diaper-check-accept ]] && rm -f /tmp/diaper-check-accept && exit 0\n");
    script.push('\n');
    script.push_str("echo \"{\\\"decision\\\":\\\"block\\\",\\\"reason\\\":$(echo \"diaper check found violations. Review and fix them, or if acceptable, run: touch /tmp/diaper-check-accept\\n\\n$OUTPUT\" | jq -Rs .)}\"\n");
    script
}

fn generate_pre_edit_hook_script() -> String {
    let diaper_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "diaper".to_string());

    let mut script = String::new();
    script.push_str("#!/bin/bash\n");
    script.push('\n');
    script.push_str("# Only run in api-gateway projects\n");
    script.push_str("[[ \"$PWD\" != *\"api-gateway\"* ]] && exit 0\n");
    script.push('\n');
    script.push_str("# Read hook input from stdin\n");
    script.push_str("INPUT=$(cat)\n");
    script.push_str("TOOL_NAME=$(echo \"$INPUT\" | jq -r '.tool_name')\n");
    script.push('\n');
    script.push_str("# Extract file path based on tool type\n");
    script.push_str("FILE_PATH=\"\"\n");
    script.push_str("if [[ \"$TOOL_NAME\" == \"Edit\" || \"$TOOL_NAME\" == \"Write\" ]]; then\n");
    script.push_str("  FILE_PATH=$(echo \"$INPUT\" | jq -r '.tool_input.file_path // empty')\n");
    script.push_str("elif [[ \"$TOOL_NAME\" == \"Bash\" ]]; then\n");
    script.push_str("  # Check if the bash command references a .js file\n");
    script.push_str("  COMMAND=$(echo \"$INPUT\" | jq -r '.tool_input.command // empty')\n");
    script.push_str("  FILE_PATH=$(echo \"$COMMAND\" | grep -oE '[^ ]*\\.js\\b' | head -1)\n");
    script.push_str("fi\n");
    script.push('\n');
    script.push_str("# Only check .js files\n");
    script.push_str("[[ -z \"$FILE_PATH\" || \"$FILE_PATH\" != *.js ]] && exit 0\n");
    script.push('\n');
    script.push_str("# Run diaper with just the missing-test rule\n");
    script.push_str(&format!("DIAPER_OUTPUT=$({diaper_bin} check --rule missing-test --json \"$FILE_PATH\" 2>/dev/null)\n"));
    script.push('\n');
    script.push_str("# If no violations, proceed silently\n");
    script.push_str("echo \"$DIAPER_OUTPUT\" | jq -e 'length > 0' > /dev/null 2>&1 || exit 0\n");
    script.push('\n');
    script.push_str("# Inject context about missing tests without blocking the edit\n");
    script.push_str("jq -n \\\n");
    script.push_str("  --arg context \"diaper: $FILE_PATH has no test file. Consider adding an index.spec.js in the same directory.\" \\\n");
    script.push_str("  '{\n");
    script.push_str("    hookSpecificOutput: {\n");
    script.push_str("      hookEventName: \"PreToolUse\",\n");
    script.push_str("      permissionDecision: \"allow\",\n");
    script.push_str("      additionalContext: $context\n");
    script.push_str("    }\n");
    script.push_str("  }'\n");
    script
}

/// Check if a hook with the given script name is already present under the given event.
fn hook_already_installed(settings: &serde_json::Value, event: &str, script_name: &str) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get(event))
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
                                .is_some_and(|cmd| cmd.contains(script_name))
                        })
                    })
            })
        })
}

/// Add a hook entry to settings under the given event.
/// Returns true if the hook was added, false if it was already present.
fn add_hook_to_settings(
    settings: &mut serde_json::Value,
    event: &str,
    script_name: &str,
    hook_entry: serde_json::Value,
) -> Result<bool, String> {
    if hook_already_installed(settings, event, script_name) {
        return Ok(false);
    }

    let hooks = settings
        .as_object_mut()
        .ok_or("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let event_hooks = hooks
        .as_object_mut()
        .ok_or("hooks is not an object")?
        .entry(event)
        .or_insert_with(|| serde_json::json!([]));

    event_hooks
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event} is not an array"))?
        .push(hook_entry);

    Ok(true)
}

/// Install all Claude Code hooks for diaper.
pub fn install_hooks() -> Result<(), String> {
    let hooks = hooks_dir();
    let settings_file = settings_path();

    install_hooks_to(&hooks, &settings_file)
}

/// Write a script file and make it executable.
fn write_script(path: &std::path::Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create hooks dir: {e}"))?;
    }

    fs::write(path, content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to chmod {}: {e}", path.display()))?;
    }

    println!("wrote {}", path.display());
    Ok(())
}

/// Core install logic, testable with arbitrary paths.
fn install_hooks_to(hooks_dir: &std::path::Path, settings_file: &std::path::Path) -> Result<(), String> {
    // 1. Write hook scripts
    let stop_script_path = hooks_dir.join(STOP_HOOK_SCRIPT);
    write_script(&stop_script_path, &generate_hook_script())?;

    let pre_edit_script_path = hooks_dir.join(PRE_EDIT_HOOK_SCRIPT);
    write_script(&pre_edit_script_path, &generate_pre_edit_hook_script())?;

    // 2. Update settings.json
    let mut settings: serde_json::Value = if settings_file.exists() {
        let contents = fs::read_to_string(settings_file)
            .map_err(|e| format!("failed to read settings.json: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse settings.json: {e}"))?
    } else {
        serde_json::json!({})
    };

    let stop_path_str = stop_script_path.to_string_lossy().to_string();
    let stop_entry = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": stop_path_str,
            "statusMessage": "Running diaper check..."
        }]
    });
    let stop_added = add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, stop_entry)?;

    let pre_edit_path_str = pre_edit_script_path.to_string_lossy().to_string();
    let pre_edit_entry = serde_json::json!({
        "matcher": "Edit|Write|Bash",
        "hooks": [{
            "type": "command",
            "command": pre_edit_path_str,
            "statusMessage": "Checking for missing tests..."
        }]
    });
    let pre_edit_added = add_hook_to_settings(&mut settings, "PreToolUse", PRE_EDIT_HOOK_SCRIPT, pre_edit_entry)?;

    if !stop_added && !pre_edit_added {
        println!("all hooks already installed in settings.json, skipping");
        return Ok(());
    }

    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize settings.json: {e}"))?;

    fs::write(settings_file, output + "\n")
        .map_err(|e| format!("failed to write settings.json: {e}"))?;

    if stop_added {
        println!("added stop hook to {}", settings_file.display());
    }
    if pre_edit_added {
        println!("added pre-edit hook to {}", settings_file.display());
    }
    println!();
    println!("diaper hooks installed:");
    println!("  - stop hook: blocks Claude on violations when finishing a task");
    println!("  - pre-edit hook: reminds Claude to add tests when editing untested files");
    println!();
    println!("tip: run Claude in bypass permissions mode so it can touch /tmp/diaper-check-accept");
    println!("     to accept violations without getting prompted:");
    println!("       claude --dangerously-skip-permissions");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helper to build a stop hook entry ---

    fn stop_hook_entry(command: &str) -> serde_json::Value {
        serde_json::json!({
            "hooks": [{
                "type": "command",
                "command": command,
                "statusMessage": "Running diaper check..."
            }]
        })
    }

    fn pre_edit_hook_entry(command: &str) -> serde_json::Value {
        serde_json::json!({
            "matcher": "Edit|Write|Bash",
            "hooks": [{
                "type": "command",
                "command": command,
                "statusMessage": "Checking for missing tests..."
            }]
        })
    }

    // --- generate_hook_script (stop hook) tests ---

    #[test]
    fn test_stop_script_contains_api_gateway_check() {
        let script = generate_hook_script();
        assert!(script.contains("api-gateway"));
    }

    #[test]
    fn test_stop_script_contains_accept_hatch() {
        let script = generate_hook_script();
        assert!(script.contains("/tmp/diaper-check-accept"));
    }

    #[test]
    fn test_stop_script_contains_json_flag() {
        let script = generate_hook_script();
        assert!(script.contains("--json"));
    }

    #[test]
    fn test_stop_script_contains_block_decision() {
        let script = generate_hook_script();
        assert!(script.contains(r#"\"decision\":\"block\""#));
    }

    #[test]
    fn test_stop_script_starts_with_shebang() {
        let script = generate_hook_script();
        assert!(script.starts_with("#!/bin/bash\n"));
    }

    #[test]
    fn test_stop_script_checks_js_files_only() {
        let script = generate_hook_script();
        assert!(script.contains("'*.js'"));
    }

    #[test]
    fn test_stop_script_captures_exit_code() {
        let script = generate_hook_script();
        assert!(script.contains("EXIT_CODE=$?"));
    }

    #[test]
    fn test_stop_script_blowouts_block_without_escape_hatch() {
        let script = generate_hook_script();
        assert!(script.contains("if [ \"$EXIT_CODE\" -ne 0 ]"));
        assert!(script.contains("BLOWOUT violations that must be fixed"));
    }

    #[test]
    fn test_stop_script_sentinel_only_for_non_blowouts() {
        let script = generate_hook_script();
        let blowout_pos = script.find("EXIT_CODE\" -ne 0").unwrap();
        let sentinel_pos = script.find("/tmp/diaper-check-accept").unwrap();
        assert!(sentinel_pos > blowout_pos);
    }

    // --- generate_pre_edit_hook_script tests ---

    #[test]
    fn test_pre_edit_script_starts_with_shebang() {
        let script = generate_pre_edit_hook_script();
        assert!(script.starts_with("#!/bin/bash\n"));
    }

    #[test]
    fn test_pre_edit_script_contains_api_gateway_check() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("api-gateway"));
    }

    #[test]
    fn test_pre_edit_script_reads_stdin_input() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("INPUT=$(cat)"));
        assert!(script.contains("tool_input.file_path"));
    }

    #[test]
    fn test_pre_edit_script_filters_js_files() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("*.js"));
    }

    #[test]
    fn test_pre_edit_script_runs_missing_test_rule() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("--rule missing-test"));
    }

    #[test]
    fn test_pre_edit_script_uses_allow_decision() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("permissionDecision"));
        assert!(script.contains("allow"));
    }

    #[test]
    fn test_pre_edit_script_uses_additional_context() {
        let script = generate_pre_edit_hook_script();
        assert!(script.contains("additionalContext"));
    }

    #[test]
    fn test_pre_edit_script_does_not_block() {
        let script = generate_pre_edit_hook_script();
        // Should not contain "block" decision — it's advisory only
        assert!(!script.contains(r#"\"decision\":\"block\""#));
        assert!(!script.contains("\"deny\""));
    }

    // --- hook_already_installed tests ---

    #[test]
    fn test_hook_already_installed_empty() {
        let settings = serde_json::json!({});
        assert!(!hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_no_event() {
        let settings = serde_json::json!({ "hooks": {} });
        assert!(!hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_empty_array() {
        let settings = serde_json::json!({ "hooks": { "Stop": [] } });
        assert!(!hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_other_hook() {
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
        assert!(!hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_stop_true() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "$HOME/.claude/hooks/diaper-check.sh"
                    }]
                }]
            }
        });
        assert!(hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_pre_edit_true() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Edit|Write|Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/path/to/diaper-pre-edit.sh"
                    }]
                }]
            }
        });
        assert!(hook_already_installed(&settings, "PreToolUse", PRE_EDIT_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_wrong_event() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/path/to/diaper-check.sh"
                    }]
                }]
            }
        });
        // Stop hook exists but we're asking about PreToolUse
        assert!(!hook_already_installed(&settings, "PreToolUse", STOP_HOOK_SCRIPT));
    }

    #[test]
    fn test_hook_already_installed_among_others() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [
                    { "hooks": [{ "type": "command", "command": "echo other" }] },
                    { "hooks": [{ "type": "command", "command": "/home/user/.claude/hooks/diaper-check.sh" }] }
                ]
            }
        });
        assert!(hook_already_installed(&settings, "Stop", STOP_HOOK_SCRIPT));
    }

    // --- add_hook_to_settings tests ---

    #[test]
    fn test_add_stop_hook_to_empty_settings() {
        let mut settings = serde_json::json!({});
        let entry = stop_hook_entry("/path/to/diaper-check.sh");
        let added = add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry).unwrap();
        assert!(added);

        let stop = &settings["hooks"]["Stop"];
        assert_eq!(stop.as_array().unwrap().len(), 1);
        assert_eq!(stop[0]["hooks"][0]["command"].as_str().unwrap(), "/path/to/diaper-check.sh");
    }

    #[test]
    fn test_add_pre_edit_hook_to_empty_settings() {
        let mut settings = serde_json::json!({});
        let entry = pre_edit_hook_entry("/path/to/diaper-pre-edit.sh");
        let added = add_hook_to_settings(&mut settings, "PreToolUse", PRE_EDIT_HOOK_SCRIPT, entry).unwrap();
        assert!(added);

        let pre = &settings["hooks"]["PreToolUse"];
        assert_eq!(pre.as_array().unwrap().len(), 1);
        assert_eq!(pre[0]["matcher"].as_str().unwrap(), "Edit|Write|Bash");
        assert_eq!(pre[0]["hooks"][0]["command"].as_str().unwrap(), "/path/to/diaper-pre-edit.sh");
    }

    #[test]
    fn test_add_both_hooks() {
        let mut settings = serde_json::json!({});
        let stop = stop_hook_entry("/path/to/diaper-check.sh");
        let pre = pre_edit_hook_entry("/path/to/diaper-pre-edit.sh");

        add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, stop).unwrap();
        add_hook_to_settings(&mut settings, "PreToolUse", PRE_EDIT_HOOK_SCRIPT, pre).unwrap();

        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_add_hook_preserves_existing_hooks() {
        let mut settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "echo existing-stop" }]
                }]
            }
        });
        let entry = stop_hook_entry("/path/to/diaper-check.sh");
        let added = add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry).unwrap();
        assert!(added);

        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing-stop");
        assert_eq!(stop[1]["hooks"][0]["command"].as_str().unwrap(), "/path/to/diaper-check.sh");
    }

    #[test]
    fn test_add_hook_preserves_non_hook_settings() {
        let mut settings = serde_json::json!({
            "permissions": { "allow": ["Bash(git *)"] },
            "model": "opus",
            "hooks": {}
        });
        let entry = stop_hook_entry("/path/to/diaper-check.sh");
        add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry).unwrap();

        assert_eq!(settings["permissions"]["allow"][0].as_str().unwrap(), "Bash(git *)");
        assert_eq!(settings["model"].as_str().unwrap(), "opus");
    }

    #[test]
    fn test_add_hook_idempotent() {
        let mut settings = serde_json::json!({});
        let entry = stop_hook_entry("/path/to/diaper-check.sh");
        add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry).unwrap();

        let entry2 = stop_hook_entry("/path/to/diaper-check.sh");
        let added = add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry2).unwrap();
        assert!(!added);
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_add_hook_preserves_other_event_hooks() {
        let mut settings = serde_json::json!({
            "hooks": {
                "PostToolUse": [{
                    "matcher": "Write|Edit",
                    "hooks": [{ "type": "command", "command": "prettier --write" }]
                }],
                "SessionStart": [{
                    "hooks": [{ "type": "command", "command": "echo hello" }]
                }]
            }
        });
        let entry = stop_hook_entry("/path/to/diaper-check.sh");
        add_hook_to_settings(&mut settings, "Stop", STOP_HOOK_SCRIPT, entry).unwrap();

        assert_eq!(settings["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    }

    // --- install_hooks_to filesystem tests ---

    #[test]
    fn test_install_creates_both_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();

        assert!(hdir.join(STOP_HOOK_SCRIPT).exists());
        assert!(hdir.join(PRE_EDIT_HOOK_SCRIPT).exists());

        let stop_contents = fs::read_to_string(hdir.join(STOP_HOOK_SCRIPT)).unwrap();
        assert!(stop_contents.starts_with("#!/bin/bash"));
        assert!(stop_contents.contains("--json"));

        let pre_edit_contents = fs::read_to_string(hdir.join(PRE_EDIT_HOOK_SCRIPT)).unwrap();
        assert!(pre_edit_contents.starts_with("#!/bin/bash"));
        assert!(pre_edit_contents.contains("--rule missing-test"));
    }

    #[test]
    fn test_install_creates_settings_with_both_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"].as_str().unwrap(), "Edit|Write|Bash");
    }

    #[test]
    fn test_install_preserves_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        let existing = serde_json::json!({
            "permissions": { "allow": ["Bash(git *)"] },
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "echo existing" }]
                }]
            },
            "model": "opus"
        });
        fs::write(&settings_path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_hooks_to(&hdir, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();

        assert_eq!(settings["permissions"]["allow"][0].as_str().unwrap(), "Bash(git *)");
        assert_eq!(settings["model"].as_str().unwrap(), "opus");
        // existing Stop preserved + new one added
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 2);
        // PreToolUse added
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_install_idempotent_does_not_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();
        install_hooks_to(&hdir, &settings_path).unwrap();
        install_hooks_to(&hdir, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&contents).unwrap();

        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_install_does_not_touch_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        fs::create_dir_all(&hdir).unwrap();
        fs::write(hdir.join("on-edit.sh"), "#!/bin/bash\necho edit").unwrap();

        let settings_path = dir.path().join("settings.json");
        install_hooks_to(&hdir, &settings_path).unwrap();

        assert_eq!(fs::read_to_string(hdir.join("on-edit.sh")).unwrap(), "#!/bin/bash\necho edit");
    }

    #[test]
    fn test_install_scripts_are_executable() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for script in &[STOP_HOOK_SCRIPT, PRE_EDIT_HOOK_SCRIPT] {
                let perms = fs::metadata(hdir.join(script)).unwrap().permissions();
                assert!(perms.mode() & 0o111 != 0, "{script} should be executable");
            }
        }
    }

    #[test]
    fn test_install_settings_is_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        let result: Result<serde_json::Value, _> = serde_json::from_str(&contents);
        assert!(result.is_ok());
    }

    #[test]
    fn test_install_settings_is_pretty_printed() {
        let dir = tempfile::tempdir().unwrap();
        let hdir = dir.path().join("hooks");
        let settings_path = dir.path().join("settings.json");

        install_hooks_to(&hdir, &settings_path).unwrap();

        let contents = fs::read_to_string(&settings_path).unwrap();
        assert!(contents.contains('\n'));
        assert!(contents.contains("  "));
    }
}
