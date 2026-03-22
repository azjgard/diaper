use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

use crate::{check, config, git};

/// Directories to ignore when watching for changes.
const IGNORED_DIRS: &[&str] = &["node_modules", "dist", "target", ".git"];

/// Returns true if the path should be ignored (inside an excluded directory).
pub fn should_ignore(path: &Path) -> bool {
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if IGNORED_DIRS.contains(&s.as_ref()) {
            return true;
        }
    }
    false
}

/// Run the check once and print results with a timestamp header.
fn run_check() {
    // Clear terminal
    print!("\x1B[2J\x1B[1;1H");

    println!("diaper watch\n");

    let files = match git::unstaged_changed_files() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error getting unstaged files: {e}");
            return;
        }
    };

    let config = config::Config::load().unwrap_or_default();
    if let Err(e) = check::check_files(&files, &config, &[]).map(|_| ()) {
        eprintln!("error: {e}");
    }
}

/// Watch the current directory for changes and re-run diaper check.
pub fn watch() -> Result<(), String> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            let dominated_by_ignored = event.paths.iter().all(|p| should_ignore(p));
            if !dominated_by_ignored {
                let _ = tx.send(());
            }
        }
    }).map_err(|e| format!("failed to create watcher: {e}"))?;

    watcher.watch(Path::new("."), RecursiveMode::Recursive)
        .map_err(|e| format!("failed to watch directory: {e}"))?;

    // Run check immediately on start
    run_check();

    let debounce = Duration::from_millis(300);
    let mut last_run = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(()) => {
                // Debounce: wait for events to settle
                let now = Instant::now();
                if now.duration_since(last_run) >= debounce {
                    // Drain any queued events
                    while rx.try_recv().is_ok() {}
                    last_run = Instant::now();
                    run_check();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events, keep waiting
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("watcher disconnected".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore_node_modules() {
        assert!(should_ignore(Path::new("node_modules/foo/bar.js")));
        assert!(should_ignore(Path::new("./node_modules/package/index.js")));
    }

    #[test]
    fn test_should_ignore_dist() {
        assert!(should_ignore(Path::new("dist/bundle.js")));
        assert!(should_ignore(Path::new("./dist/index.js")));
    }

    #[test]
    fn test_should_ignore_target() {
        assert!(should_ignore(Path::new("target/debug/diaper")));
    }

    #[test]
    fn test_should_ignore_git() {
        assert!(should_ignore(Path::new(".git/objects/abc123")));
    }

    #[test]
    fn test_should_not_ignore_src() {
        assert!(!should_ignore(Path::new("src/main.rs")));
    }

    #[test]
    fn test_should_not_ignore_root_js() {
        assert!(!should_ignore(Path::new("index.js")));
    }

    #[test]
    fn test_should_not_ignore_nested_src() {
        assert!(!should_ignore(Path::new("src/rules/file_too_long.rs")));
    }

    #[test]
    fn test_should_ignore_deeply_nested_node_modules() {
        assert!(should_ignore(Path::new("packages/app/node_modules/foo/bar.js")));
    }

    #[test]
    fn test_should_not_ignore_similar_name() {
        // "node_modules_backup" should not be ignored
        assert!(!should_ignore(Path::new("node_modules_backup/foo.js")));
    }

    #[test]
    fn test_should_not_ignore_dist_in_name() {
        // "distribution" should not be ignored
        assert!(!should_ignore(Path::new("distribution/foo.js")));
    }
}
