mod check;
mod config;
mod git;
mod hook;
mod rules;
mod watch;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "diaper", about = "Score JavaScript files for code smells")]
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
    }
}
