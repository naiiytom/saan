use anyhow::Result;
use saan_core::{SqlDialect, ShaverRegistry, Store};
use std::path::Path;

pub fn run(input: &Path, store_path: &Path, dialect: SqlDialect) -> Result<()> {
    if !store_path.exists() {
        anyhow::bail!(
            "Store not found at '{}'. Run `saan init` first.",
            store_path.display()
        );
    }
    let store = Store::open(store_path)?;
    let registry = ShaverRegistry::with_builtins().with_sql_dialect(dialect);
    let strands = registry.shave_path(input)?;

    let file_count = strands.len();
    let node_count: usize = strands.iter().map(|s| s.nodes.len()).sum();
    let edge_count: usize = strands.iter().map(|s| s.edges.len()).sum();

    store.write_strands_to_staging(&strands)?;
    println!(
        "Staged: {} file(s), {} node(s), {} edge(s)",
        file_count, node_count, edge_count
    );
    Ok(())
}
