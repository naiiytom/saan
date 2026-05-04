use anyhow::{Context as _, Result, bail};
use saan_core::{QueryResult, Store};
use std::path::Path;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Csv,
    Json,
}

pub fn run(sql: &str, store_path: &Path, format: &OutputFormat) -> Result<()> {
    if !store_path.exists() {
        bail!(
            "store not found at {}; run `saan init` first",
            store_path.display()
        );
    }
    let store = Store::open(store_path)
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;
    let result = store.query(sql)?;

    match format {
        OutputFormat::Table => print_table(&result),
        OutputFormat::Csv => print_csv(&result),
        OutputFormat::Json => print_json(&result),
    }

    Ok(())
}

fn print_table(result: &QueryResult) {
    if result.columns.is_empty() {
        return;
    }
    let mut widths: Vec<usize> = result.columns.iter().map(|c| c.len()).collect();
    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let separator: String = widths
        .iter()
        .map(|w| "-".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("+");
    let separator = format!("+{}+", separator);

    let header: String = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| format!(" {:width$} ", c, width = widths[i]))
        .collect::<Vec<_>>()
        .join("|");
    let header = format!("|{}|", header);

    println!("{separator}");
    println!("{header}");
    println!("{separator}");
    for row in &result.rows {
        let line: String = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                format!(
                    " {:width$} ",
                    row.get(i).map(|s| s.as_str()).unwrap_or(""),
                    width = w
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        println!("|{}|", line);
    }
    println!("{separator}");
    println!("{} row(s)", result.rows.len());
}

fn print_csv(result: &QueryResult) {
    println!("{}", csv_row(&result.columns));
    for row in &result.rows {
        println!("{}", csv_row(row));
    }
}

fn csv_row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains(',') || f.contains('"') || f.contains('\n') {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn print_json(result: &QueryResult) {
    let rows: Vec<String> = result
        .rows
        .iter()
        .map(|row| {
            let fields: Vec<String> = result
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    format!("\"{}\":{}", json_escape(col), json_value(val))
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        })
        .collect();
    println!("[{}]", rows.join(","));
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn json_value(s: &str) -> String {
    // Emit as JSON string for all values (keeps types predictable without a schema).
    format!("\"{}\"", json_escape(s))
}
