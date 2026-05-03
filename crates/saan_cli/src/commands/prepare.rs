use anyhow::{Context, Result};
use saan_core::{ShaverRegistry, Store};
use std::path::Path;

pub fn run(input: &Path, store_path: &Path) -> Result<()> {
    let store = Store::open(store_path)
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;

    let registry = ShaverRegistry::with_builtins();
    let strands = registry
        .shave_path(input)
        .with_context(|| format!("failed to process {}", input.display()))?;

    let node_count: usize = strands.iter().map(|s| s.nodes.len()).sum();
    let edge_count: usize = strands.iter().map(|s| s.edges.len()).sum();

    store.write_strands_to_staging(&strands)?;
    println!(
        "Prepared {} file(s): {} node(s), {} edge(s) written to staging",
        strands.len(),
        node_count,
        edge_count,
    );
    Ok(())
}
