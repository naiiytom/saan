mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "saan", version, about = "Saan — metadata lineage platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new .saan metadata store
    Init {
        /// Directory in which to create .saan (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Overwrite an existing store
        #[arg(long)]
        force: bool,
    },
    /// Extract metadata from source files into the staging tables
    Prepare {
        /// Input directory or file to walk
        input: PathBuf,
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Promote staging tables into the final graph
    Apply {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Define lineage connections [not implemented in Phase 1]
    Interlace,
    /// Validate the graph structure [not implemented in Phase 1]
    Inspect,
    /// Launch the WASM visualizer [not implemented in Phase 1]
    View,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path, force } => commands::init::run(&path, force)?,
        Commands::Prepare { input, store } => commands::prepare::run(&input, &store)?,
        Commands::Apply { store } => commands::apply::run(&store)?,
        Commands::Interlace | Commands::Inspect | Commands::View => {
            eprintln!("not implemented in Phase 1");
            std::process::exit(1);
        }
    }

    Ok(())
}
