# Saan — Agent Context

This file gives AI agents working on this repo the context they need to contribute effectively.

## What Saan Is

Saan is a **metadata lineage and visualization platform** written in Rust. You point it at SQL files, dbt manifests, or other data-asset sources; it extracts how things connect, persists a directed acyclic graph, lets you validate the structure, and renders a "Woven Mesh" browser visualizer. Ad-hoc DuckDB query of the same store is a planned later surface (Phase 5).

## Docs

- `docs/TECHNICAL_SPECIFICATIONS.md` — architecture reference, component responsibilities, tech stack
- `docs/ROADMAP.md` — six-phase delivery plan; Phase 1 (lineage spine MVP) is active
- `docs/IMPLEMENTATION_PLAN.md` — detailed work breakdown for Phase 1 only: crate layout, Cargo deps, public API surface, SqlShaver behavior, `.saan` schema, testing strategy, 13-step implementation order

Read the IMPLEMENTATION_PLAN before making Phase 1 changes.

## Architecture

```
saan_core    (The Weaver + The Shaving Layer) — public Rust library
saan_cli     (The Toolbelt) — binary, thin orchestrator over saan_core
saan_mesh    (The Mesh) — WASM/React visualizer, unchanged in Phase 1
sdk/python   (The Hand) — PyO3/maturin stubs, unchanged in Phase 1
```

### Core Components

**The Weaver (`saan_core`)** — graph management, lineage extraction, store persistence.

**The Shaving Layer** — Shavers are structs that implement the `Shaver` trait. Each Shaver reads one input type and returns a `Strand` (extracted nodes + edges). `ShaverRegistry` dispatches by file extension. The CLI calls `ShaverRegistry::with_builtins()`.

**The Store** — DuckDB embedded database persisted as a `.saan` file. Has five tables: `nodes`, `edges`, `staging_nodes`, `staging_edges`, `saan_meta`.

**The Mesh** — standalone WASM+React app. Untouched in Phase 1.

**The Hand** — Python SDK (PyO3 bindings). Untouched in Phase 1.

## Crate Layout (Target — Phase 1)

```
crates/saan_core/src/
    lib.rs                  re-exports public surface
    graph.rs                Node, Edge, Graph (petgraph wrapper)
    strand.rs               Strand type
    shaver/
        mod.rs              Shaver trait, ShaverError, ShaverRegistry
        sql.rs              SqlShaver (only built-in for Phase 1)
    store.rs                DuckDB persistence

crates/saan_cli/src/
    main.rs                 clap entry point + dispatch
    commands/
        mod.rs
        init.rs
        prepare.rs
        apply.rs
```

## Public Library Surface (saan_core)

```rust
pub trait Shaver: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn shave(&self, input: &Path) -> Result<Strand, ShaverError>;
}

pub struct Strand {
    pub source_path: PathBuf,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

pub enum ShaverError { Io { path, source }, Parse { path, message }, Other(String) }

pub struct ShaverRegistry { ... }
impl ShaverRegistry {
    pub fn new() -> Self;
    pub fn with_builtins() -> Self;     // SqlShaver pre-registered for .sql
    pub fn register(&mut self, shaver: Arc<dyn Shaver>) -> &mut Self;
    pub fn for_extension(&self, ext: &str) -> Option<&Arc<dyn Shaver>>;
}
```

## CLI Commands

| Command | Phase 1 status |
|---|---|
| `saan init [path] [--force]` | Implemented — creates `.saan` file |
| `saan prepare <input> [--store path]` | Implemented — walks input, runs Shavers, writes staging |
| `saan apply [--store path]` | Implemented — UPSERTs staging → final tables |
| `saan interlace` | Stub — prints "not implemented in Phase 1" and exits 1 |
| `saan inspect` | Stub — same |
| `saan view` | Stub — same |

## `.saan` File Schema (DuckDB)

```sql
saan_meta(key PK, value)                          -- schema_version = '1'
nodes(id PK, label, source_type, first_seen_at, last_seen_at)
edges(from_id, to_id, PK(from_id, to_id))
staging_nodes(id, label, source_type, source_path)
staging_edges(from_id, to_id, source_path)
```

No FK from edges to nodes — cross-system orphan edges are valid; `inspect` (Phase 2) reports them.

## SqlShaver Behavior

Handles: `CREATE TABLE AS SELECT`, `CREATE VIEW AS SELECT`, `INSERT INTO ... SELECT`, bare `SELECT`.

Correctness rules:
- **CTEs excluded** — pre-walk `WITH` clause, substitute CTE refs with their real-table upstreams.
- **Subqueries propagate** — sources inside FROM-list subqueries flow through to outer target.
- **Qualified names preserved** — `prod.raw.orders` stays as that id; not collapsed to `orders`.
- **Quoted identifiers preserve case** — `"My Table"` → `My Table`; unquoted → lowercased.
- Dialect: `GenericDialect` (sqlparser-rs). Per-Shaver dialect config deferred to Phase 2.

Not handled in Phase 1: `MERGE`, stored procedures, dynamic SQL (`EXECUTE`), Jinja/dbt macros, `TRUNCATE`/`DROP`.

## Key Design Decisions

- **Staging-then-apply** — `prepare` writes to staging tables; `apply` UPSERTs into final tables and truncates staging. This preserves the spec's three-phase model (prepare → interlace → apply) so Phase 2 `interlace` inserts between them without surgery.
- **Sync `Shaver` trait** — no `async`. A separate `AsyncShaver` for cloud sources arrives in Phase 6. Don't retrofit.
- **`bundled` duckdb feature** — contributors do not need a system DuckDB installation.
- **No FK on edges** — lineage regularly references nodes from systems not yet ingested. Orphans are a Phase 2 `inspect` concern, not a schema constraint.
- **Trait from day one** — `Shaver` is a public library surface because saan_core is intended to be used as a library. External crates implement `Shaver` and register through `ShaverRegistry`.

## Dependencies

| Crate | dep | Purpose |
|---|---|---|
| `saan_core` | `petgraph = "0.6"` | DAG management, cycle detection |
| `saan_core` | `sqlparser = "0.50"` | SQL AST parsing |
| `saan_core` | `duckdb = { version = "1", features = ["bundled"] }` | Store |
| `saan_core` | `thiserror = "1"` | Error types |
| `saan_core` | `walkdir = "2"` | Directory traversal |
| `saan_core` | `serde`, `serde_json` | Serialization (future use) |
| `saan_cli` | `clap = { version = "4", features = ["derive"] }` | Argument parsing |
| `saan_cli` | `anyhow = "1"` | CLI error propagation |

## Testing Conventions

- Unit tests colocated in each module (`#[cfg(test)]` blocks at the bottom of each file).
- Integration tests in `crates/saan_core/tests/` — full library pipeline against fixture `.sql` files in `tests/fixtures/sql/`.
- CLI integration tests in `crates/saan_cli/tests/cli_integration.rs` using `assert_cmd`.
- DuckDB tests use tempfiles (`tempfile = "3"`) — never a fixed path.
- **Idempotence is the key property to test** — same input through `prepare` + `apply` twice must add zero new rows.
- `cargo test --workspace` must pass on every commit.

## Out of Scope for Phase 1

Do not implement: `interlace`/`inspect`/`view` logic, Python SDK, WASM changes, `AsyncShaver`, additional Shavers (dbt, Parquet, BI), per-dialect config, plugin discovery, performance tuning. Add CLI stubs that exit non-zero with "not implemented in Phase 1" — never panic or leave them missing.

## Running Locally

```bash
cargo build          # builds all crates (first build with DuckDB takes ~5-10 min)
cargo test           # run unit tests
cargo test --workspace   # run all tests including CLI integration
```
