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
    let added = store.interlace_staging()?;
    println!("Interlaced: {} computed edge(s) added to staging.", added);
    Ok(())
}
