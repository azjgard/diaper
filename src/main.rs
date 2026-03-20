mod check;
mod config;
mod git;
mod hook;
mod rules;
mod watch;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "diaper", version, about = "Score JavaScript files for code smells")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check files for code smells
    Check {
        /// File path to check (if omitted, checks unstaged git changes)
        path: Option<String>,
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },
    /// Watch for file changes and re-run checks
    Watch,
    /// Generate a default diaper.yml config file
    Init,
    /// Install Claude Code stop hook (blocks Claude on violations, use with claude --dangerously-skip-permissions)
    InstallHook,
    /// List all rules (or show details for a specific rule)
    Rules {
        /// Rule name to show details for
        name: Option<String>,
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

fn print_rule_detail(name: &str) {
    let all = rules::all_rules();
    let rule = match all.iter().find(|r| r.name() == name) {
        Some(r) => r,
        None => {
            eprintln!("error: unknown rule '{name}'");
            eprintln!("Run 'diaper rules' to see all available rules.");
            std::process::exit(1);
        }
    };

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

#[cfg(test)]
mod tests {
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { path, json } => {
            let config = match config::Config::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            };

            let files = match path {
                Some(p) => vec![p],
                None => match git::unstaged_changed_files() {
                    Ok(files) => files,
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                },
            };

            let result = if json {
                check::check_files_json(&files, &config)
            } else {
                check::check_files(&files, &config)
            };

            match result {
                Ok(has_smells) => {
                    if has_smells {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Watch => {
            if let Err(e) = watch::watch() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Init => {
            if let Err(e) = config::init_config() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Commands::InstallHook => {
            if let Err(e) = hook::install_hook() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Rules { name } => {
            match name {
                Some(n) => print_rule_detail(&n),
                None => print_rules_list(),
            }
        }
    }
}
