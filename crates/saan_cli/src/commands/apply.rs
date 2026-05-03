use anyhow::{Context, Result, bail};
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
    store.apply_staging()?;

    let g = store.load_graph()?;
    println!(
        "Applied staging: {} node(s), {} edge(s) in graph",
        g.node_count(),
        g.edge_count(),
    );
    Ok(())
}
