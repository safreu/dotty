use std::path::PathBuf;

use crate::commands::{completions, forget, init, ls, manage, test};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

mod application;
mod commands;
mod domain;
mod infrastructure;
mod presentation;

#[derive(Parser)]
#[command(name = "dotty", about = "Dotfile management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // Takes a path pointing to dotfiles directory,
    // if it doesn't exist yet, one will be created.
    // Init will create a config toml where this path will be written to
    // this will be written to $XDG_CONFIG_HOME/dotty/config.toml,
    // in case its not set it, it will be written to .config/dotty/config.toml
    Init {
        #[arg(long)]
        // This path will be used to store the dotfiles, in case it doesn't exist yet it will be created
        repo_dir_path: PathBuf,
    },
    Manage {
        #[arg(long, value_hint = ValueHint::AnyPath)]
        target: PathBuf,

        #[arg(long = "as", value_hint = ValueHint::AnyPath)]
        source: Option<PathBuf>,
    },

    Forget {
        #[arg(long, value_hint = ValueHint::AnyPath)]
        target: PathBuf,
    },

    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },

    Test {
        path: Option<PathBuf>,
    },
    Ls,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { repo_dir_path } => match init::execute(repo_dir_path) {
            Ok(_) => {
                println!("Init success");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed with: {e}");
                std::process::ExitCode::from(2)
            }
        },
        Commands::Manage { target, source } => match manage::execute(target, source) {
            Ok(_) => {
                println!("manage success");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed with: {e}");
                std::process::ExitCode::from(2)
            }
        },
        Commands::Forget { target } => match forget::execute(target) {
            Ok(_) => {
                println!("forget success");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed with: {e}");
                std::process::ExitCode::from(2)
            }
        },
        Commands::Completions { shell } => {
            completions::execute(shell, Cli::command());
            std::process::ExitCode::SUCCESS
        }
        Commands::Test { path } => match test::execute(path) {
            Ok(true) => std::process::ExitCode::SUCCESS,
            Ok(false) => std::process::ExitCode::from(1),
            Err(error) => {
                eprintln!("Failed to inspect dotfiles: {error}");
                std::process::ExitCode::from(2)
            }
        },
        Commands::Ls => match ls::execute() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Failed to list managed entries: {error}");
                std::process::ExitCode::from(2)
            }
        },
    }
}
