use anyhow::{Context as _, Result, bail};
use saan_core::{Store, SvgConfig, SvgRenderer, wrap_svg_in_html};
use std::path::Path;

pub fn run(store_path: &Path, out_path: &Path) -> Result<()> {
    if !store_path.exists() {
        bail!(
            "store not found at {}; run `saan init` first",
            store_path.display()
        );
    }
    let store = Store::open(store_path)
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;
    let graph = store.load_graph()?;

    let svg = SvgRenderer::render(&graph, &SvgConfig::default());
    let html = wrap_svg_in_html(&svg, "saan lineage");

    std::fs::write(out_path, &html)
        .with_context(|| format!("failed to write output to {}", out_path.display()))?;

    println!(
        "Written: {} ({} node(s), {} edge(s))",
        out_path.display(),
        graph.node_count(),
        graph.edge_count(),
    );
    Ok(())
}
