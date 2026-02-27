# Roadmap

## Phase 1: Foundation (Core & Ingestion)

*   [ ] **OmniParser:**
    *   [ ] Basic file ingestion (CSV, JSON, Parquet).
    *   [ ] DuckDB connection management.
    *   [ ] Simple SQL query execution.
*   [ ] **Artisan Toolbelt:**
    *   [ ] CLI skeleton with `ingest` command.
    *   [ ] Configuration file parsing (TOML).
    *   [ ] Logging and error handling.
*   [ ] **Testing:**
    *   [ ] Unit tests for OmniParser.
    *   [ ] Integration tests for ingestion flow.

## Phase 2: CLI & Basic Visualization

*   [ ] **Artisan Toolbelt:**
    *   [ ] `serve` command to host a REST API.
    *   [ ] API endpoint to query DuckDB and return JSON/Arrow.
*   [ ] **Woven Mesh UI:**
    *   [ ] Basic WASM application (e.g., Yew/Leptos).
    *   [ ] Fetch data from Artisan Toolbelt API.
    *   [ ] Render a simple data table.
*   [ ] **Documentation:**
    *   [ ] Update README with usage guide.

## Phase 3: Advanced UI & SDK Integration

*   [ ] **Woven Mesh UI:**
    *   [ ] Interactive charts and graphs (e.g., using Plotters or JS interop).
    *   [ ] SQL editor with syntax highlighting.
    *   [ ] Client-side filtering and sorting.
*   [ ] **Python SDK:**
    *   [ ] PyO3 bindings for OmniParser core logic.
    *   [ ] `OmniClient` class for interacting with local DB or remote server.
    *   [ ] Pandas/Polars conversion utilities.

## Phase 4: Optimization & Ecosystem

*   [ ] **Performance:**
    *   [ ] Optimize large dataset ingestion.
    *   [ ] Implement streaming processing where applicable.
    *   [ ] Reduce WASM binary size.
*   [ ] **Ecosystem:**
    *   [ ] Plugin system for custom data sources.
    *   [ ] Cloud deployment scripts (Docker, Kubernetes).
    *   [ ] Community examples and tutorials.
