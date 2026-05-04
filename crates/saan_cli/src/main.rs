mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use saan_core::SqlDialect;

#[derive(Debug, Clone, clap::ValueEnum)]
enum CliDialect {
    Generic,
    Ansi,
    Postgres,
    Mysql,
    Mssql,
    Bigquery,
    Snowflake,
    Hive,
    Redshift,
    Sqlite,
    Duckdb,
    Clickhouse,
}

impl From<CliDialect> for SqlDialect {
    fn from(d: CliDialect) -> Self {
        match d {
            CliDialect::Generic    => SqlDialect::Generic,
            CliDialect::Ansi       => SqlDialect::Ansi,
            CliDialect::Postgres   => SqlDialect::Postgres,
            CliDialect::Mysql      => SqlDialect::MySql,
            CliDialect::Mssql      => SqlDialect::MsSql,
            CliDialect::Bigquery   => SqlDialect::BigQuery,
            CliDialect::Snowflake  => SqlDialect::Snowflake,
            CliDialect::Hive       => SqlDialect::Hive,
            CliDialect::Redshift   => SqlDialect::Redshift,
            CliDialect::Sqlite     => SqlDialect::SQLite,
            CliDialect::Duckdb     => SqlDialect::DuckDb,
            CliDialect::Clickhouse => SqlDialect::ClickHouse,
        }
    }
}

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
        /// SQL dialect for parsing
        #[arg(long, value_enum, default_value = "generic")]
        dialect: CliDialect,
    },
    /// Promote staging tables into the final graph
    Apply {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Define lineage connections [not implemented in Phase 1]
    Interlace,
    /// Validate the graph structure
    Inspect {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Launch the WASM visualizer [not implemented in Phase 1]
    View,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path, force } => commands::init::run(&path, force)?,
        Commands::Prepare { input, store, dialect } => {
            commands::prepare::run(&input, &store, dialect.into())?
        }
        Commands::Apply { store } => commands::apply::run(&store)?,
        Commands::Interlace => {
            eprintln!("not implemented in Phase 1");
            std::process::exit(1);
        }
        Commands::Inspect { store } => commands::inspect::run(&store)?,
        Commands::View => {
            eprintln!("not implemented in Phase 1");
            std::process::exit(1);
        }
    }

    Ok(())
}
