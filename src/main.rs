mod check;
mod git;
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { path, json } => {
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
                check::check_files_json(&files)
            } else {
                check::check_files(&files)
            };

            if let Err(e) = result {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Watch => {
            if let Err(e) = watch::watch() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}
