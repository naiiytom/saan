# Implementation Plan — Phase 1 Lineage MVP

**Status:** Complete  
**Branch:** `feat/lineage-mvp` (PR #2)  
**Scope:** The Phase 1 milestone in `ROADMAP.md` — the thinnest end-to-end lineage spine: `saan init` + `saan prepare` (with one `SqlShaver`) + `saan apply`, persisting a real graph to a real `.saan` file.

This plan is the work breakdown for Phase 1 only. Phase 2+ get their own plans when their phase begins.

---

## 1. Goal

A user can do this from a fresh clone:

```powershell
cargo build --bin saan_cli
mkdir my-pipeline
saan_cli init my-pipeline
echo "CREATE TABLE marts.orders AS SELECT * FROM raw.orders" > my-pipeline/model.sql
saan_cli prepare my-pipeline --store my-pipeline/.saan
saan_cli apply --store my-pipeline/.saan
# Prints: Applied staging: 2 node(s), 1 edge(s) in graph
```

**Achieved.** All success criteria in Section 9 met. 51 tests pass (`cargo test --workspace`).

## 2. Out of Scope (Explicit)

To prevent scope creep during implementation:

- `interlace`, `inspect`, `view` command implementations — keep their CLI parser entries, exit with "not implemented in Phase 1".
- Python SDK / PyO3 bindings (Phase 4).
- WASM mesh / visualizer changes — `saan_mesh` is unchanged this phase.
- Async runtime (Tokio) (Phase 6).
- Additional Shavers — dbt, Parquet, JSON, BI exports (Phase 6).
- Per-Shaver dialect configuration (Phase 2).
- Streaming SQL parsing.
- Plugin discovery (Phase 6).
- Performance tuning, parallel ingestion.
- Migration tooling for `saan_meta.schema_version` — we are at v1; no migrations needed yet.

## 3. Crate-Level Changes

### 3.1 `saan_core` (the Weaver — public library)

Module layout as implemented:

```
saan_core/src/
    lib.rs              — re-exports the public surface
    graph.rs            — Node, Edge, Graph (petgraph wrapper)
    strand.rs           — Strand type
    shaver/
        mod.rs          — Shaver trait, ShaverError, ShaverRegistry
        sql.rs          — SqlShaver (built-in)
    store.rs            — DuckDB persistence
```

Dependencies in `crates/saan_core/Cargo.toml`:

```toml
[dependencies]
petgraph = "0.6"
sqlparser = "0.50"
duckdb = { version = "1", features = ["bundled"] }
thiserror = "1"
walkdir = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tempfile = "3"
```

The `bundled` feature on `duckdb` is mandatory — contributors must not need a system DuckDB.

### 3.2 `saan_cli` (the Toolbelt)

Layout as implemented:

```
saan_cli/src/
    main.rs             — clap entry point + dispatch
    commands/
        mod.rs
        init.rs
        prepare.rs
        apply.rs
```

Dependencies in `crates/saan_cli/Cargo.toml`:

```toml
[dependencies]
saan_core = { path = "../saan_core" }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

### 3.3 `saan_mesh`

Unchanged this phase.

## 4. Public Library Surface

### 4.1 `Shaver` trait

```rust
pub trait Shaver: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn shave(&self, input: &Path) -> Result<Strand, ShaverError>;
}
```

### 4.2 `Strand`

```rust
pub struct Strand {
    pub source_path: PathBuf,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Strand {
    pub fn new(source_path: PathBuf) -> Self;
}
```

**Deviation from plan:** The plan included `add_node` / `add_edge` builder methods. Implemented with direct public field access (`strand.nodes.push(...)`) — simpler and sufficient for current usage.

### 4.3 `ShaverError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum ShaverError {
    #[error("IO error reading {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{0}")]
    Other(String),
}
```

### 4.4 `ShaverRegistry`

```rust
pub struct ShaverRegistry {
    shavers: HashMap<String, Arc<dyn Shaver>>,
}

impl ShaverRegistry {
    pub fn new() -> Self;
    pub fn with_builtins() -> Self;  // SqlShaver pre-registered for .sql
    pub fn register(&mut self, shaver: Arc<dyn Shaver>) -> &mut Self;
    pub fn for_extension(&self, ext: &str) -> Option<&Arc<dyn Shaver>>;
    pub fn shave_path(&self, input: &Path) -> Result<Vec<Strand>, ShaverError>;
}
```

**Addition vs plan:** `shave_path()` walks a directory (or single file) and dispatches to registered Shavers. Moved here so `saan_cli` does not need `walkdir` as a direct dependency.

**API decisions worth preserving:**

- **Sync, not async.** Async would force every consumer to depend on a runtime. `AsyncShaver` arrives separately in Phase 6.
- **`&Path` input.** Streaming variant deferred until a real use case appears.
- **Concrete `ShaverError` enum**, not generic `E` — keeps `Box<dyn Shaver>` painless.

## 5. `SqlShaver` Behavior

### 5.1 SQL constructs handled

| Statement | Behavior |
|---|---|
| `CREATE TABLE t AS SELECT ... FROM a, b` | Node `t`, edges `a → t`, `b → t` |
| `CREATE VIEW v AS SELECT ... FROM a` | Node `v`, edge `a → v` |
| `INSERT INTO t SELECT ... FROM a` | Edge `a → t` |
| Bare `SELECT ... FROM a` | Node `a` only, no edges |

### 5.2 Correctness rules

- **CTEs are not upstream nodes.** Pre-walk the `WITH` clause, collect CTE names per scope, compute each CTE's transitive upstream set. Substitute CTE references with their real-table upstreams when resolving outer `FROM` / `JOIN`.
- **Subqueries propagate.** Sources inside a FROM-list subquery flow through to the outer target.
- **Schema/database qualification preserved.** `prod.raw.orders` stays as that exact id.
- **Quoted identifiers preserve case.** `"My Table"` → `My Table`. Unquoted → lowercased.

### 5.3 Non-goals (Phase 2+)

- `MERGE`, `UPSERT`, multi-target DML.
- Stored procedures, function bodies, EXECUTE / dynamic SQL.
- Jinja templating (`{{ ref('x') }}`) — dbt Shaver in Phase 6.
- Cross-statement temp tables — process per-statement; treat temp refs as opaque.
- `TRUNCATE`, `DROP` — no lineage to extract.

### 5.4 Dialect

Hard-coded to sqlparser-rs's `GenericDialect`. Per-Shaver dialect config goes in Phase 2.

### 5.5 File-level behavior

- Read as UTF-8. Non-UTF-8 → `ShaverError::Io`.
- Parse as a statement list. Parse error → `ShaverError::Parse { path, message }` with sqlparser's line/column rolled into the message.
- All nodes/edges from all statements in a file land in one Strand.

### 5.6 Node fields produced

- `id`: normalized canonical name (lowercase unless quoted; dotted form preserved).
- `label`: same as `id` (Phase 2 `interlace` lets users override).
- `source_type`: `"sql"`.

### 5.7 Idempotence

Same SQL file shaved twice produces identical Strands. The staging-then-apply UPSERT means re-running `prepare` over the same input is a no-op for `apply`.

## 6. Data Flow

### 6.1 `saan init [path] [--force]`

- `path` is a directory; store file is created at `<path>/.saan`. Default: current directory.
- Refuses if file exists; `--force` overwrites.
- Opens DuckDB at path, runs schema DDL (Section 7), closes.

### 6.2 `saan prepare <input> [--store path]`

- Default store: `./.saan`.
- Walks `<input>` recursively (also accepts a single file) via `ShaverRegistry::shave_path`.
- Per file: look up Shaver by extension. Unknown extensions silently skipped.
- End of run: write all Strands into `staging_nodes` / `staging_edges`. Final `nodes` / `edges` not touched.
- Prints: files processed, node rows staged, edge rows staged.

### 6.3 `saan apply [--store path]`

- Default store: `./.saan`.
- One `execute_batch`: UPSERT staging into final `nodes` / `edges`, then `DELETE FROM` staging.
- Timestamp behavior: on insert, `first_seen_at = last_seen_at = now()`. On conflict (id exists), `first_seen_at` preserved; `last_seen_at = now()`.
- Prints: node count and edge count in final graph.

### 6.4 Why staging-then-apply (not direct write)

The spec models lineage construction as three steps (prepare → interlace → apply). `interlace` is a no-op in MVP, but staging preserves the pipeline shape without surgery later.

## 7. `.saan` File Schema (DuckDB)

```sql
CREATE TABLE IF NOT EXISTS saan_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO saan_meta (key, value) VALUES ('schema_version', '1')
    ON CONFLICT (key) DO NOTHING;

CREATE TABLE IF NOT EXISTS nodes (
    id            TEXT PRIMARY KEY,
    label         TEXT NOT NULL,
    source_type   TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at  TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    from_id TEXT NOT NULL,
    to_id   TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id)
);

CREATE TABLE IF NOT EXISTS staging_nodes (
    id          TEXT NOT NULL,
    label       TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS staging_edges (
    from_id     TEXT NOT NULL,
    to_id       TEXT NOT NULL,
    source_path TEXT NOT NULL
);
```

**Deviations from plan:**

- Column type is `TEXT` (DuckDB alias) not `VARCHAR` — functionally identical, matches duckdb-rs defaults.
- Timestamps are `TIMESTAMPTZ` not `TIMESTAMP` — timezone-aware.
- `source_path` is `NOT NULL` — every staging row must know where it came from.
- `ON CONFLICT DO NOTHING` on the `saan_meta` insert instead of plain `INSERT` — makes `init_schema` idempotent.
- `DELETE FROM` used instead of `TRUNCATE` — broader DuckDB compatibility in `execute_batch`.
- `current_timestamp` in `ON CONFLICT DO UPDATE` is treated as a column reference by DuckDB; use `now()` instead.

**Two deliberate non-decisions:**

- **No FK from `edges` to `nodes`.** Lineage routinely points at nodes from systems not yet ingested. `inspect` (Phase 2) reports orphans, but they are not a constraint failure.
- **`source_path` is metadata, not identity.** Two SQL files referencing `raw.orders` produce one node. The Shaver normalizes; `source_path` traces which file contributed which staging row.

## 8. Testing Strategy

### 8.1 Unit tests (colocated with each module)

- `graph` — all original cases preserved against petgraph-backed `Graph`; cycle detection via `is_cyclic_directed`.
- `shavers::sql` — 10 cases: CREATE TABLE AS, CREATE VIEW AS, INSERT INTO, bare SELECT, JOIN captures both sides, CTE not exposed as node, subquery sources propagate, qualified names, quoted-identifier case, unquoted lowercased, UNION captures both branches.
- `store` — idempotent `init_schema`, write-and-apply round-trip, double-apply is no-op, staging cleared after apply.

### 8.2 Integration tests (`crates/saan_core/tests/`)

Against fixture files in `tests/fixtures/sql/`:
- `orders_pipeline.sql` — multi-statement, JOIN; verifies node presence and edge direction.
- `with_cte.sql` — nested CTEs; verifies CTE names absent from nodes.
- Full store round-trip (prepare → apply → load_graph).
- Idempotence via double prepare+apply.
- Original four pipeline tests (node count, edge count, acyclic, cycle detection).

### 8.3 CLI integration tests (`crates/saan_cli/tests/cli_integration.rs`)

- No-args exits non-zero, stderr contains "Usage".
- `--help` exits zero, stdout lists all six subcommands.
- Unknown command exits non-zero.
- `init` creates `.saan`; second `init` without `--force` fails with "already exists"; `--force` succeeds.
- `prepare` without input arg fails.
- `interlace`, `inspect`, `view` exit non-zero with "not implemented".
- Full end-to-end pipeline test with inline SQL fixture.
- Idempotence test: prepare+apply twice, final apply reports same counts.

## 9. Success Criteria

All criteria met:

1. ✅ `cargo test --workspace` passes (51 tests).
2. ✅ The fresh-clone flow in Section 1 produces the expected output.
3. ✅ Re-running `prepare` + `apply` on the same input adds zero new rows.
4. ✅ Phase-2 commands exit cleanly with "not implemented in Phase 1" — no panics.
5. ✅ `ROADMAP.md` and `TECHNICAL_SPECIFICATIONS.md` aligned with this plan.

## 10. Implementation Order (Completed)

1. ✅ Wire deps — petgraph, sqlparser, duckdb, thiserror, walkdir, clap, anyhow.
2. ✅ Replace Graph — petgraph `StableDiGraph` wrapper; existing test suite preserved.
3. ✅ `Strand`, `ShaverError`, `Shaver` trait, `ShaverRegistry`.
4. ✅ `SqlShaver` — bare SELECT, CREATE TABLE/VIEW AS, INSERT, CTEs, subqueries, qualified names, case handling.
5. ✅ `Store` — schema: `open`, `init_schema`, all 5 tables.
6. ✅ `Store` — staging writes: `write_strands_to_staging`.
7. ✅ `Store` — apply: `apply_staging` UPSERT logic.
8. ✅ `Store` — load_graph: round-trip test.
9. ✅ CLI — clap skeleton; phase-2 stubs exit 1 with "not implemented".
10. ✅ CLI — `init` wired to Store.
11. ✅ CLI — `prepare` wired to ShaverRegistry + Store staging.
12. ✅ CLI — `apply` wired to Store::apply_staging.
13. ✅ End-to-end CLI integration test including idempotence.

## 11. Build Notes (Windows)

The GNU `ld` linker (`x86_64-pc-windows-gnu`) cannot link DuckDB's large bundled static library — it exits with code 5 (internal error). Fixed by:

- **`rust-toolchain.toml`** — pins `stable-x86_64-pc-windows-msvc` so `link.exe` is used instead.
- **`.cargo/config.toml`** — adds `rstrtmgr.lib` to the MSVC link step; DuckDB uses Windows Restart Manager APIs (`RmStartSession` etc.) that are not linked by default.

Linux/macOS are unaffected: the `rstrtmgr.lib` flag is scoped to `[target.x86_64-pc-windows-msvc]` and the GNU toolchain issue does not exist on those platforms. Linux contributors should remove or override `rust-toolchain.toml` (e.g. `rustup override set stable`).
