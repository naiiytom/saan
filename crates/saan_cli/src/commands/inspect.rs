use anyhow::{Context as _, Result, bail};
use saan_core::Store;
use std::path::Path;

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
        if report.orphan_nodes.is_empty() {
            "none".to_string()
        } else {
            report.orphan_nodes.join(", ")
        }
    );
    println!(
        "External refs ({}): {}",
        report.external_refs.len(),
        if report.external_refs.is_empty() {
            "none".to_string()
        } else {
            report.external_refs.join(", ")
        }
    );

    Ok(())
}
