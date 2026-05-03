use anyhow::{Result, bail};
use saan_core::Store;
use std::path::Path;

pub fn run(dir: &Path, force: bool) -> Result<()> {
    let store_path = dir.join(".saan");

    if store_path.exists() {
        if force {
            std::fs::remove_file(&store_path)?;
        } else {
            bail!(
                "{} already exists; use --force to overwrite",
                store_path.display()
            );
        }
    }

    let store = Store::open(&store_path)?;
    store.init_schema()?;
    println!("Initialized {}", store_path.display());
    Ok(())
}
