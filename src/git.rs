use std::process::Command;

/// Returns a list of unstaged changed file paths from the current git repo.
/// Includes both modified and untracked files.
pub fn unstaged_changed_files() -> Result<Vec<String>, String> {
    // Get modified but unstaged files
    let modified = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=d"])
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;

    if !modified.status.success() {
        let stderr = String::from_utf8_lossy(&modified.stderr);
        return Err(format!("git diff failed: {stderr}"));
    }

    // Get untracked files
    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .map_err(|e| format!("failed to run git ls-files: {e}"))?;

    if !untracked.status.success() {
        let stderr = String::from_utf8_lossy(&untracked.stderr);
        return Err(format!("git ls-files failed: {stderr}"));
    }

    let mut files: Vec<String> = Vec::new();

    let modified_str = String::from_utf8_lossy(&modified.stdout);
    for line in modified_str.lines() {
        if !line.is_empty() {
            files.push(line.to_string());
        }
    }

    let untracked_str = String::from_utf8_lossy(&untracked.stdout);
    for line in untracked_str.lines() {
        if !line.is_empty() && !files.contains(&line.to_string()) {
            files.push(line.to_string());
        }
    }

    Ok(files)
}

/// Returns .js files changed between the given git ref and HEAD.
pub fn diff_files(git_ref: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["diff", &format!("{git_ref}...HEAD"), "--name-only", "--diff-filter=d", "--", "*.js"])
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff failed: {stderr}"));
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    Ok(files)
}

/// Detect the repo name from the git remote URL.
/// Parses `git remote -v` and extracts the repo name from the origin URL.
/// Returns None if not in a git repo or no origin remote found.
pub fn detect_repo() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "-v"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        // Match origin remote (fetch or push)
        if line.starts_with("origin") {
            return extract_repo_name(line);
        }
    }

    None
}

/// Extract repo name from a git remote line.
/// Handles both SSH and HTTPS formats:
///   origin	git@github.com:org/repo-name.git (fetch)
///   origin	https://github.com/org/repo-name.git (fetch)
fn extract_repo_name(line: &str) -> Option<String> {
    // Get the URL part (second whitespace-separated token)
    let url = line.split_whitespace().nth(1)?;

    // Strip trailing .git if present
    let url = url.strip_suffix(".git").unwrap_or(url);

    // Get the last path segment (the repo name)
    let name = if url.contains(':') && !url.contains("://") {
        // SSH format: git@github.com:org/repo-name
        url.rsplit('/').next().or_else(|| url.rsplit(':').next())
    } else {
        // HTTPS format: https://github.com/org/repo-name
        url.rsplit('/').next()
    };

    name.map(|n| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unstaged_changed_files_runs_without_panic() {
        // We're in a git repo, so this should at least not error
        let result = unstaged_changed_files();
        assert!(result.is_ok());
    }

    #[test]
    fn test_unstaged_changed_files_returns_vec() {
        let files = unstaged_changed_files().unwrap();
        // Just verify it returns a vec of strings (may or may not have items)
        assert!(files.iter().all(|f| !f.is_empty()));
    }

    #[test]
    fn test_extract_repo_name_ssh() {
        let line = "origin\tgit@github.com:applause-hq/api-gateway.git (fetch)";
        assert_eq!(extract_repo_name(line), Some("api-gateway".to_string()));
    }

    #[test]
    fn test_extract_repo_name_https() {
        let line = "origin\thttps://github.com/applause-hq/api-gateway.git (fetch)";
        assert_eq!(extract_repo_name(line), Some("api-gateway".to_string()));
    }

    #[test]
    fn test_extract_repo_name_no_git_suffix() {
        let line = "origin\tgit@github.com:applause-hq/integration-hub (fetch)";
        assert_eq!(extract_repo_name(line), Some("integration-hub".to_string()));
    }

    #[test]
    fn test_extract_repo_name_https_no_git_suffix() {
        let line = "origin\thttps://github.com/applause-hq/integration-hub (push)";
        assert_eq!(extract_repo_name(line), Some("integration-hub".to_string()));
    }

    #[test]
    fn test_detect_repo_returns_something() {
        // We're in a git repo with an origin remote
        let result = detect_repo();
        // May or may not have an origin, so just check it doesn't panic
        let _ = result;
    }
}
