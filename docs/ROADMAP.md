# Roadmap

Saan is a **lineage-first** platform: extract how data assets connect, persist the graph, validate it, visualize it. Ad-hoc query (`SELECT ... FROM nodes`-style usage of the underlying store) is a deliberate later surface that builds on the lineage spine — not a parallel product.

Each phase ends in a usable artifact. No half-finished surfaces leak between phases.

## Phase 1 — Lineage Spine (MVP)

The thinnest end-to-end slice that proves the lineage premise. See `IMPLEMENTATION_PLAN.md` for the detailed work breakdown.

*   [ ] **`saan_core`:**
    *   [ ] Replace custom `Graph` with a `petgraph::stable_graph::StableDiGraph` wrapper.
    *   [ ] `Strand` type — bundle of nodes/edges produced by one shave.
    *   [ ] Public `Shaver` trait + `ShaverError` + `ShaverRegistry`.
    *   [ ] `SqlShaver` (built-in): handles `CREATE TABLE AS`, `CREATE VIEW AS`, `INSERT INTO ... SELECT`, bare `SELECT`, with correct CTE / subquery / qualified-name handling. Uses `sqlparser-rs` (Generic dialect).
    *   [ ] `Store` — DuckDB-backed `.saan` file: `open`, `init_schema`, `write_strands_to_staging`, `apply_staging`, `load_graph`.
*   [ ] **`saan_cli`:**
    *   [ ] Adopt `clap` for argument parsing.
    *   [ ] `saan init [path] [--force]` — create empty `.saan` file.
    *   [ ] `saan prepare <input> [--store path]` — walk inputs, dispatch to Shavers, write Strands to staging.
    *   [ ] `saan apply [--store path]` — UPSERT staging into final tables in one transaction.
    *   [ ] `saan interlace` / `inspect` / `view` — keep parser entries, exit non-zero with "not implemented in Phase 1".
*   [ ] **Testing:**
    *   [ ] Unit tests for `graph`, `strand`, `shaver`, `shavers::sql`, `store` (idempotence is the key property).
    *   [ ] Integration tests for the full library pipeline against fixture `.sql` files.
    *   [ ] CLI integration tests for `init && prepare && apply`, idempotent re-runs, and "not implemented" exits.

## Phase 2 — Validation

*   [ ] **`saan interlace`:** automated edge-building beyond raw `FROM`/`JOIN` — semantic CTE elaboration, dbt `ref()` resolution, deeper subquery handling.
*   [ ] **`saan inspect`:** structural reports — orphan nodes, cycles, edges pointing at unknown nodes, schema drift.
*   [ ] Per-Shaver dialect configuration (Snowflake, Postgres, BigQuery, etc.).

## Phase 3 — Visualization

*   [ ] **`saan view` (HTML+SVG):** compile the graph into a single static HTML file with an inline SVG render. No server, no WASM yet — just something a user can open in a browser.
*   [ ] **`saan_mesh` (WASM React):** the full "Woven Mesh" interactive visualizer. Replaces the static HTML for interactive exploration.

## Phase 4 — Python SDK

*   [ ] PyO3 bindings via `maturin` for the public `saan_core` surface (`Store`, `Shaver`, `ShaverRegistry`, `Graph`).
*   [ ] `saan_ops.connect(path)` returns a real `SaanConnection` backed by the Rust core.
*   [ ] `prepare`, `apply`, `interlace`, `inspect`, `view` mirror the CLI.

## Phase 5 — Ad-Hoc Query

*   [ ] **`saan query "SELECT ..."`** — expose The Store via a SQL passthrough on the CLI.
*   [ ] **`db.query(sql).to_pandas()` / `.to_polars()` / `.to_arrow()`** — Python SDK convenience.
*   [ ] Query layer is thin: it reads from the same DuckDB store that lineage writes to. No separate ingestion path.

## Phase 6 — Ecosystem & Additional Shavers

*   [ ] Additional Shavers: dbt manifests, JSON/YAML/TOML manifests, Parquet/CSV blob inspection, BI tool exports (Tableau, PowerBI).
*   [ ] Async Shaver trait (`AsyncShaver`) for cloud DBs / APIs (Snowflake, BigQuery, REST endpoints). Built on Tokio.
*   [ ] Plugin system: load external Shaver crates at runtime.
*   [ ] Performance: parallel ingestion, streaming SQL parsing, large-graph optimization, WASM binary size reduction.
*   [ ] Deployment: Docker images, Kubernetes manifests, CI examples.
*   [ ] Community examples and tutorials.
