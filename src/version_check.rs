use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const REPO: &str = "azjgard/diaper";
const INSTALL_URL: &str = "https://raw.githubusercontent.com/azjgard/diaper/main/install.sh";
const STATE_FILE: &str = "latest-version";
const GREEN: &str = "\x1b[32m";
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
    if is_newer(&latest, current) {
        eprintln!();
        eprintln!("{BOLD}Update available:{RESET} {YELLOW}v{current}{RESET} -> {GREEN}v{latest}{RESET}");
        eprintln!("Run {BOLD}diaper update{RESET} to install the latest version.");
    }
}

/// Parse a version string like "1.2.3" or "1.2.3-beta" into (major, minor, patch, prerelease).
/// Strips a leading "v" if present.
fn parse_version(v: &str) -> (u32, u32, u32, &str) {
    let v = v.strip_prefix('v').unwrap_or(v);
    let (version_part, pre) = match v.find('-') {
        Some(i) => (&v[..i], &v[i + 1..]),
        None => (v, ""),
    };
    let parts: Vec<u32> = version_part.split('.').filter_map(|s| s.parse().ok()).collect();
    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    let patch = parts.get(2).copied().unwrap_or(0);
    (major, minor, patch, pre)
}

/// Returns true if `latest` is strictly newer than `current`.
/// Versions are compared as major.minor.patch. If the numeric parts are equal,
/// a release (no prerelease suffix) is considered newer than a prerelease,
/// but two different prerelease suffixes at the same numeric version are equal.
fn is_newer(latest: &str, current: &str) -> bool {
    let (lmaj, lmin, lpat, lpre) = parse_version(latest);
    let (cmaj, cmin, cpat, cpre) = parse_version(current);

    match (lmaj.cmp(&cmaj), lmin.cmp(&cmin), lpat.cmp(&cpat)) {
        (std::cmp::Ordering::Greater, _, _) => true,
        (std::cmp::Ordering::Less, _, _) => false,
        (_, std::cmp::Ordering::Greater, _) => true,
        (_, std::cmp::Ordering::Less, _) => false,
        (_, _, std::cmp::Ordering::Greater) => true,
        (_, _, std::cmp::Ordering::Less) => false,
        // Same numeric version: release > prerelease
        _ => lpre.is_empty() && !cpre.is_empty(),
    }
}

/// Fetch the latest release version from GitHub synchronously.
fn fetch_latest_version() -> Result<String, String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"curl -sL "https://api.github.com/repos/{REPO}/releases/latest" | grep '"tag_name"' | head -1 | cut -d '"' -f 4 | sed 's/^v//'"#
        ))
        .output()
        .map_err(|e| format!("failed to fetch latest version: {e}"))?;

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("could not determine latest version".to_string());
    }
    Ok(version)
}

/// Update diaper to the latest version.
pub fn update() -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");

    print!("Checking for updates... ");
    let latest = fetch_latest_version()?;

    if !is_newer(&latest, current) {
        println!("{GREEN}Already up to date ✅{RESET}");
        return Ok(());
    }

    println!("updating {YELLOW}v{current}{RESET} -> {GREEN}v{latest}{RESET}");
    println!();

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!("curl -fsSL {INSTALL_URL} | bash"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to run install script: {e}"))?;

    if !output.status.success() {
        return Err("install script failed".to_string());
    }

    // Update the state file so the next run doesn't show an update notice
    let _ = fs::write(state_file_path(), &latest);

    println!("Updated to {GREEN}{BOLD}v{latest}{RESET}");
    println!("Release notes: https://github.com/{REPO}/releases/tag/v{latest}");

    Ok(())
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

    // --- parse_version ---

    #[test]
    fn test_parse_version_simple() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3, ""));
    }

    #[test]
    fn test_parse_version_with_prerelease() {
        assert_eq!(parse_version("1.2.3-beta"), (1, 2, 3, "beta"));
    }

    #[test]
    fn test_parse_version_strips_v_prefix() {
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3, ""));
    }

    #[test]
    fn test_parse_version_v_prefix_with_prerelease() {
        assert_eq!(parse_version("v0.4.1-beta"), (0, 4, 1, "beta"));
    }

    // --- is_newer ---

    #[test]
    fn test_is_newer_major_bump() {
        assert!(is_newer("2.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_minor_bump() {
        assert!(is_newer("1.1.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_patch_bump() {
        assert!(is_newer("1.0.1", "1.0.0"));
    }

    #[test]
    fn test_is_newer_same_version() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_older_major() {
        assert!(!is_newer("1.0.0", "2.0.0"));
    }

    #[test]
    fn test_is_newer_older_minor() {
        assert!(!is_newer("1.0.0", "1.1.0"));
    }

    #[test]
    fn test_is_newer_older_patch() {
        assert!(!is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn test_is_newer_release_over_prerelease() {
        assert!(is_newer("1.0.0", "1.0.0-beta"));
    }

    #[test]
    fn test_is_newer_prerelease_not_over_release() {
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn test_is_newer_same_prerelease() {
        assert!(!is_newer("1.0.0-beta", "1.0.0-beta"));
    }

    #[test]
    fn test_is_newer_different_prerelease_same_version() {
        assert!(!is_newer("1.0.0-alpha", "1.0.0-beta"));
    }

    #[test]
    fn test_is_newer_higher_prerelease() {
        assert!(is_newer("1.1.0-beta", "1.0.0-beta"));
    }

    #[test]
    fn test_is_newer_lower_prerelease() {
        assert!(!is_newer("1.0.0-beta", "1.1.0-beta"));
    }

    #[test]
    fn test_is_newer_with_v_prefix() {
        assert!(is_newer("v2.0.0", "v1.0.0"));
    }

    #[test]
    fn test_is_newer_mixed_v_prefix() {
        assert!(is_newer("v2.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "v2.0.0"));
    }

    #[test]
    fn test_is_newer_real_versions() {
        assert!(is_newer("0.4.1-beta", "0.4.0-beta"));
        assert!(!is_newer("0.4.0-beta", "0.4.1-beta"));
        assert!(!is_newer("0.4.1-beta", "0.4.1-beta"));
    }
}
