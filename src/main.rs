mod check;
mod git;
mod rules;

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
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { path } => {
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

            if let Err(e) = check::check_files(&files) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}
