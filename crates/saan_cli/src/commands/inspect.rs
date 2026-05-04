use anyhow::{Context as _, Result, bail};
use saan_core::Store;
use std::path::Path;

const LIST_LIMIT: usize = 10;

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let mut s = items[..items.len().min(LIST_LIMIT)].join(", ");
    if items.len() > LIST_LIMIT {
        s.push_str(&format!(" ...and {} more", items.len() - LIST_LIMIT));
    }
    s
}

pub fn run(store_path: &Path) -> Result<()> {
    if !store_path.exists() {
        bail!(
            "store not found at {}; run `saan init` first",
            store_path.display()
        );
    }
    let store = Store::open(store_path)
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;
    let report = store.inspect()?;

    println!("=== saan inspect ===");
    println!("Nodes:  {}", report.total_nodes);
    println!("Edges:  {}", report.total_edges);
    println!();

    if report.cycle_detected {
        println!("WARNING: cycle detected in graph");
    }

    println!(
        "Orphan nodes  ({}): {}",
        report.orphan_nodes.len(),
        format_list(&report.orphan_nodes),
    );
    println!(
        "External refs ({}): {}",
        report.external_refs.len(),
        format_list(&report.external_refs),
    );

    Ok(())
}
