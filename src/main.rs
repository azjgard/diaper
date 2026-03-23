mod check;
mod config;
mod git;
mod hook;
mod rules;
mod version_check;
mod watch;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "diaper", version, about = "Score JavaScript files for code smells", disable_version_flag = true)]
struct Cli {
    /// Print version
    #[arg(short = 'v', short_alias = 'V', long = "version", action = clap::ArgAction::Version)]
    version: (),

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check files for code smells
    Check {
        /// Files or directories to check (if omitted, checks unstaged git changes)
        paths: Vec<String>,
        /// Output results as JSON
        #[arg(long)]
        json: bool,
        /// Only run specific rules (repeatable or comma-separated)
        #[arg(long = "rule", short = 'r', value_delimiter = ',')]
        rules: Vec<String>,
    },
    /// Check files changed between a git ref and HEAD
    CheckDiff {
        /// Git ref to diff against (branch, tag, or commit)
        #[arg(name = "ref")]
        git_ref: String,
        /// Output results as JSON
        #[arg(long)]
        json: bool,
        /// Only run specific rules (repeatable or comma-separated)
        #[arg(long = "rule", short = 'r', value_delimiter = ',')]
        rules: Vec<String>,
    },
    /// Watch for file changes and re-run checks
    Watch,
    /// Generate a default diaper.yml config file
    Init,
    /// Install Claude Code hooks (stop hook + pre-edit test reminder)
    InstallHooks,
    /// Update diaper to the latest version
    Update,
    /// List all rules (or show details for a specific rule)
    Rules {
        /// Rule name to show details for
        name: Option<String>,
        /// Show full details for all rules
        #[arg(long, short)]
        verbose: bool,
    },
}

const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

fn print_rules_list() {
    let all = rules::all_rules();
    println!("{BOLD}diaper rules{RESET} ({} total)\n", all.len());
    for rule in &all {
        println!("  {CYAN}{}{RESET} {DIM}-{RESET} {}", rule.name(), rule.description());
    }
    println!("\n{DIM}Run 'diaper rules <name>' for details and examples.{RESET}");
}

fn print_rule_detail_by_name(name: &str) {
    let all = rules::all_rules();
    let rule = match all.iter().find(|r| r.name() == name) {
        Some(r) => r,
        None => {
            eprintln!("error: unknown rule '{name}'");
            eprintln!("Run 'diaper rules' to see all available rules.");
            std::process::exit(1);
        }
    };
    print_rule_detail(rule.as_ref());
}

fn print_rule_detail(rule: &dyn rules::Rule) {
    println!("{BOLD}{CYAN}{}{RESET}", rule.name());
    println!("{}", rule.description());
    println!("{DIM}score: {}{RESET}", rule.default_score());
    println!("{DIM}{}{RESET}", rule.doc_url());

    let (bad, good) = rule.examples();

    if !bad.is_empty() {
        println!("\n{RED}{BOLD}Bad:{RESET}");
        for example in bad {
            for line in example.lines() {
                println!("  {RED}{line}{RESET}");
            }
        }
    }

    if !good.is_empty() {
        println!("\n{GREEN}{BOLD}Good:{RESET}");
        for example in good {
            for line in example.lines() {
                println!("  {GREEN}{line}{RESET}");
            }
        }
    }
}

fn print_rules_verbose() {
    let all = rules::all_rules();
    println!("{BOLD}diaper rules{RESET} ({} total)\n", all.len());
    for (i, rule) in all.iter().enumerate() {
        if i > 0 {
            println!("\n{DIM}---{RESET}\n");
        }
        print_rule_detail(rule.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn test_version_flag() {
        let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("diaper");
        let output = Command::new(&bin)
            .arg("--version")
            .output()
            .expect("failed to run diaper — run `cargo build` first");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.starts_with("diaper "));
        assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_collect_js_files_finds_js() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.js"), "const x = 1;").unwrap();
        fs::write(dir.path().join("b.js"), "const y = 2;").unwrap();
        fs::write(dir.path().join("c.txt"), "not js").unwrap();

        let files = collect_js_files(dir.path().to_str().unwrap());
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.ends_with(".js")));
    }

    #[test]
    fn test_collect_js_files_recurses_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("top.js"), "1;").unwrap();
        fs::write(sub.join("nested.js"), "2;").unwrap();

        let files = collect_js_files(dir.path().to_str().unwrap());
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.contains("nested.js")));
    }

    #[test]
    fn test_collect_js_files_skips_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let nm = dir.path().join("node_modules");
        fs::create_dir(&nm).unwrap();
        fs::write(dir.path().join("app.js"), "1;").unwrap();
        fs::write(nm.join("lib.js"), "2;").unwrap();

        let files = collect_js_files(dir.path().to_str().unwrap());
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("app.js"));
    }

    #[test]
    fn test_collect_js_files_skips_dot_git() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        fs::create_dir(&git).unwrap();
        fs::write(dir.path().join("app.js"), "1;").unwrap();
        fs::write(git.join("hook.js"), "2;").unwrap();

        let files = collect_js_files(dir.path().to_str().unwrap());
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_collect_js_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let files = collect_js_files(dir.path().to_str().unwrap());
        assert!(files.is_empty());
    }

    #[test]
    fn test_collect_js_files_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("z.js"), "1;").unwrap();
        fs::write(dir.path().join("a.js"), "2;").unwrap();
        fs::write(dir.path().join("m.js"), "3;").unwrap();

        let files = collect_js_files(dir.path().to_str().unwrap());
        assert_eq!(files.len(), 3);
        assert!(files[0] < files[1]);
        assert!(files[1] < files[2]);
    }
}

/// Recursively collect all `.js` files under a directory, skipping ignored dirs.
fn collect_js_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(dir)];

    while let Some(current) = dirs.pop() {
        let entries = match std::fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !watch::should_ignore(&path) {
                    dirs.push(path);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("js") {
                if let Some(s) = path.to_str() {
                    files.push(s.to_string());
                }
            }
        }
    }

    files.sort();
    files
}

fn run() -> i32 {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { paths, json, rules: rule_filter } => {
            let config = match config::Config::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            };

            if !rule_filter.is_empty() {
                let all = rules::all_rules();
                for name in &rule_filter {
                    if !all.iter().any(|r| r.name() == name.as_str()) {
                        eprintln!("error: unknown rule '{name}'");
                        eprintln!("Run 'diaper rules' to see all available rules.");
                        return 1;
                    }
                }
            }

            let files = if paths.is_empty() {
                match git::unstaged_changed_files() {
                    Ok(files) => files,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 1;
                    }
                }
            } else {
                let mut all_files = Vec::new();
                for p in paths {
                    let meta = std::fs::metadata(&p);
                    if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                        all_files.extend(collect_js_files(&p));
                    } else {
                        all_files.push(p);
                    }
                }
                all_files
            };

            let result = if json {
                check::check_files_json(&files, &config, &rule_filter)
            } else {
                check::check_files(&files, &config, &rule_filter)
            };

            match result {
                Ok(has_smells) => {
                    if has_smells {
                        return 1;
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            }
        }
        Commands::CheckDiff { git_ref, json, rules: rule_filter } => {
            let config = match config::Config::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            };

            if !rule_filter.is_empty() {
                let all = rules::all_rules();
                for name in &rule_filter {
                    if !all.iter().any(|r| r.name() == name.as_str()) {
                        eprintln!("error: unknown rule '{name}'");
                        eprintln!("Run 'diaper rules' to see all available rules.");
                        return 1;
                    }
                }
            }

            let files = match git::diff_files(&git_ref) {
                Ok(files) => files,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            };

            let result = if json {
                check::check_files_json(&files, &config, &rule_filter)
            } else {
                check::check_files(&files, &config, &rule_filter)
            };

            match result {
                Ok(has_smells) => {
                    if has_smells {
                        return 1;
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            }
        }
        Commands::Watch => {
            if let Err(e) = watch::watch() {
                eprintln!("error: {e}");
                return 1;
            }
        }
        Commands::Init => {
            if let Err(e) = config::init_config() {
                eprintln!("error: {e}");
                return 1;
            }
        }
        Commands::InstallHooks => {
            if let Err(e) = hook::install_hooks() {
                eprintln!("error: {e}");
                return 1;
            }
        }
        Commands::Update => {
            if let Err(e) = version_check::update() {
                eprintln!("error: {e}");
                return 1;
            }
        }
        Commands::Rules { name, verbose } => {
            match name {
                Some(n) => print_rule_detail_by_name(&n),
                None if verbose => print_rules_verbose(),
                None => print_rules_list(),
            }
        }
    }

    0
}

fn main() {
    version_check::spawn_update_check();
    let exit_code = run();
    version_check::print_update_notice();
    std::process::exit(exit_code);
}
