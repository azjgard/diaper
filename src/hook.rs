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

fn hook_script_path() -> PathBuf {
    hooks_dir().join(HOOK_SCRIPT_NAME)
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

/// Install the Claude Code Stop hook for diaper.
pub fn install_hook() -> Result<(), String> {
    // 1. Write the hook script
    let script_path = hook_script_path();
    let hooks_dir = hooks_dir();

    fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("failed to create hooks dir: {e}"))?;

    fs::write(&script_path, generate_hook_script())
        .map_err(|e| format!("failed to write hook script: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to chmod hook script: {e}"))?;
    }

    println!("wrote {}", script_path.display());

    // 2. Update settings.json
    let settings_path = settings_path();
    let mut settings: serde_json::Value = if settings_path.exists() {
        let contents = fs::read_to_string(&settings_path)
            .map_err(|e| format!("failed to read settings.json: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse settings.json: {e}"))?
    } else {
        serde_json::json!({})
    };

    if hook_already_installed(&settings) {
        println!("stop hook already installed in settings.json, skipping");
        return Ok(());
    }

    // Build the hook entry
    let hook_entry = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": script_path.to_string_lossy(),
            "statusMessage": "Running diaper check..."
        }]
    });

    // Ensure hooks.Stop exists and append
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

    // Write back with pretty formatting
    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize settings.json: {e}"))?;

    fs::write(&settings_path, output + "\n")
        .map_err(|e| format!("failed to write settings.json: {e}"))?;

    println!("added stop hook to {}", settings_path.display());
    println!("\ndiaper will now check for violations when Claude stops in {PROJECT_DIR}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
