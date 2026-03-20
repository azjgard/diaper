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
}
