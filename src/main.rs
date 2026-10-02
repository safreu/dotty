use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::commands::{init, manage};

mod commands;
mod config;
mod handlers;

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
        dotfiles_dir_path: PathBuf,
    },
    // Dotty will manage itself, meaning after init, program will call dotty manage 'self'
    Manage {
        #[arg(long)]
        target: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { dotfiles_dir_path } => match init::execute(dotfiles_dir_path) {
            Ok(_) => println!("Init success"),
            Err(e) => println!("Failed with {}", e),
        },
        Commands::Manage { target } => match manage::execute(target) {
            Ok(_) => println!("manage success"),
            Err(e) => println!("Failed with {}", e),
        },
    }
}
