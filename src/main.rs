use std::path::PathBuf;

use crate::commands::{completions, forget, init, manage};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

mod application;
mod commands;
mod domain;
mod infrastructure;
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
    // Dotty will manage itself, meaning after init, program will call dotty manage 'self'
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { repo_dir_path } => match init::execute(repo_dir_path) {
            Ok(_) => println!("Init success"),
            Err(e) => println!("Failed with {}", e),
        },
        Commands::Manage { target, source } => match manage::execute(target, source) {
            Ok(_) => println!("manage success"),
            Err(e) => println!("Failed with {}", e),
        },
        Commands::Forget { target } => match forget::execute(target) {
            Ok(_) => println!("forget success"),
            Err(e) => println!("Failed with {}", e),
        },
        Commands::Completions { shell } => completions::execute(shell, Cli::command()),
    }
}
