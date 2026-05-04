mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::query::OutputFormat;
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
    /// Compute derived lineage edges in staging
    Interlace {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Validate the graph structure
    Inspect {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
    },
    /// Render the lineage graph to a self-contained HTML file
    View {
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
        /// Output HTML file path
        #[arg(long, default_value = "lineage.html")]
        out: PathBuf,
    },
    /// Run an ad-hoc SQL query against the store
    Query {
        /// SQL statement to execute
        sql: String,
        /// Path to the .saan store
        #[arg(long, default_value = ".saan")]
        store: PathBuf,
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path, force } => commands::init::run(&path, force)?,
        Commands::Prepare { input, store, dialect } => {
            commands::prepare::run(&input, &store, dialect.into())?
        }
        Commands::Apply { store } => commands::apply::run(&store)?,
        Commands::Interlace { store } => commands::interlace::run(&store)?,
        Commands::Inspect { store } => commands::inspect::run(&store)?,
        Commands::View { store, out } => commands::view::run(&store, &out)?,
        Commands::Query { sql, store, format } => commands::query::run(&sql, &store, &format)?,
    }

    Ok(())
}
