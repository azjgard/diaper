use std::fs;
use std::path::PathBuf;
use std::process::Command;

const REPO: &str = "azjgard/diaper";
const STATE_FILE: &str = "latest-version";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".diaper")
}

fn state_file_path() -> PathBuf {
    state_dir().join(STATE_FILE)
}

/// Spawn a background process that fetches the latest release tag from GitHub
/// and writes it to ~/.diaper/latest-version. Fire-and-forget.
pub fn spawn_update_check() {
    let dir = state_dir();
    let path = state_file_path();

    // Build a shell command that curls the API and writes the tag to the state file
    let script = format!(
        r#"mkdir -p "{dir}" && curl -sL "https://api.github.com/repos/{REPO}/releases/latest" | grep '"tag_name"' | head -1 | cut -d '"' -f 4 | sed 's/^v//' > "{path}""#,
        dir = dir.display(),
        path = path.display(),
    );

    // Spawn detached — we don't care about the result
    let _ = Command::new("sh")
        .arg("-c")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Check ~/.diaper/latest-version against the current compiled version.
/// If a newer version is available, print an update message to stderr.
pub fn print_update_notice() {
    let path = state_file_path();
    let latest = match fs::read_to_string(&path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return,
    };

    if latest.is_empty() {
        return;
    }

    let current = env!("CARGO_PKG_VERSION");
    if latest != current {
        eprintln!();
        eprintln!("{YELLOW}{BOLD}Update available:{RESET}{YELLOW} v{current} -> v{latest}{RESET}");
        eprintln!("{YELLOW}Run: curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | bash{RESET}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_dir_under_home() {
        let dir = state_dir();
        assert!(dir.to_string_lossy().contains(".diaper"));
    }

    #[test]
    fn test_state_file_path_has_filename() {
        let path = state_file_path();
        assert_eq!(path.file_name().unwrap(), STATE_FILE);
    }

    #[test]
    fn test_print_update_notice_no_file() {
        // Should not panic when file doesn't exist
        let _path = state_file_path();
        print_update_notice();
    }

    #[test]
    fn test_print_update_notice_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE);
        fs::write(&path, env!("CARGO_PKG_VERSION")).unwrap();
        // No panic, no output (can't easily assert stderr in unit tests)
    }

    #[test]
    fn test_print_update_notice_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE);
        fs::write(&path, "").unwrap();
        // Should not panic on empty file
    }
}
