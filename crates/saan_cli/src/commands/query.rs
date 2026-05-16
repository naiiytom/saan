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
    store
        .init_schema()
        .with_context(|| "failed to initialise store schema")?;
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
            if f.contains(',') || f.contains('"') || f.contains('\n') || f.contains('\r') {
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
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_value(s: &str) -> String {
    // Emit as JSON string for all values (keeps types predictable without a schema).
    format!("\"{}\"", json_escape(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_row_plain_values_joined_with_comma() {
        let row = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(csv_row(&row), "a,b,c");
    }

    #[test]
    fn csv_row_value_with_comma_is_quoted() {
        let row = vec!["hello, world".to_string()];
        assert_eq!(csv_row(&row), "\"hello, world\"");
    }

    #[test]
    fn csv_row_value_with_double_quote_is_escaped_and_quoted() {
        let row = vec!["say \"hi\"".to_string()];
        assert_eq!(csv_row(&row), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_row_value_with_newline_is_quoted() {
        let row = vec!["line1\nline2".to_string()];
        let out = csv_row(&row);
        assert!(out.starts_with('"') && out.ends_with('"'));
    }

    #[test]
    fn csv_row_value_with_carriage_return_is_quoted() {
        let row = vec!["line1\rline2".to_string()];
        let out = csv_row(&row);
        assert!(out.starts_with('"') && out.ends_with('"'));
    }

    #[test]
    fn csv_row_empty_slice_produces_empty_string() {
        assert_eq!(csv_row(&[]), "");
    }

    #[test]
    fn json_escape_plain_string_is_unchanged() {
        assert_eq!(json_escape("hello"), "hello");
    }

    #[test]
    fn json_escape_backslash_is_doubled() {
        assert_eq!(json_escape("a\\b"), "a\\\\b");
    }

    #[test]
    fn json_escape_double_quote_is_escaped() {
        assert_eq!(json_escape("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn json_escape_newline_and_tab() {
        assert_eq!(json_escape("a\nb\tc"), "a\\nb\\tc");
    }

    #[test]
    fn json_escape_carriage_return() {
        assert_eq!(json_escape("a\rb"), "a\\rb");
    }

    #[test]
    fn json_escape_control_char_uses_unicode_escape() {
        // ASCII 0x01 is a control character
        let out = json_escape("\x01");
        assert_eq!(out, "\\u0001");
    }

    #[test]
    fn json_value_wraps_in_double_quotes() {
        assert_eq!(json_value("foo"), "\"foo\"");
    }

    #[test]
    fn json_value_escapes_inner_quotes() {
        assert_eq!(json_value("say \"hi\""), "\"say \\\"hi\\\"\"");
    }
}
