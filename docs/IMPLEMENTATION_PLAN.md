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

---

# Implementation Plan — Phase 2 Validation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `saan interlace` (transitive edge computation), `saan inspect` (structural validation reports), and per-Shaver SQL dialect configuration.

**Architecture:** Three independent features layered onto the Phase 1 spine: (1) a `SqlDialect` enum lets users configure the SQL parser dialect per-prepare run; (2) `Store::inspect()` queries the final graph for orphan nodes, cycles, and external refs; (3) `Store::interlace_staging()` performs BFS-based transitive closure on staging edges before apply.

**Tech Stack:** Rust, duckdb-rs 1.x, sqlparser-rs 0.50, clap 4, petgraph 0.6, tempfile (tests), assert_cmd + predicates (CLI tests).

---

## File Map

| File | Status | Responsibility |
|------|--------|----------------|
| `crates/saan_core/src/shaver/sql.rs` | Modify | Add `SqlDialect` enum; give `SqlShaver` a `dialect` field and constructors |
| `crates/saan_core/src/shaver/mod.rs` | Modify | Update `with_builtins()` for new constructor; add `with_sql_dialect()` |
| `crates/saan_core/src/store.rs` | Modify | Add `InspectReport` struct; add `Store::inspect()` and `Store::interlace_staging()` |
| `crates/saan_core/src/lib.rs` | Modify | Re-export `SqlDialect` and `InspectReport` |
| `crates/saan_cli/src/main.rs` | Modify | Add `CliDialect` enum + `From` impl; update `Prepare`/`Interlace`/`Inspect` subcommands; update dispatch |
| `crates/saan_cli/src/commands/prepare.rs` | Modify | Accept `SqlDialect` param; switch to `ShaverRegistry::with_sql_dialect()` |
| `crates/saan_cli/src/commands/mod.rs` | Modify | Add `pub mod interlace; pub mod inspect;` |
| `crates/saan_cli/src/commands/interlace.rs` | Create | `run(store_path)` → `store.interlace_staging()` |
| `crates/saan_cli/src/commands/inspect.rs` | Create | `run(store_path)` → `store.inspect()`, format report |
| `crates/saan_cli/tests/cli_integration.rs` | Modify | Replace "not implemented" stubs for interlace/inspect; add dialect, inspect, interlace tests |

---

## Task 1: SqlShaver dialect configuration

**Files:**
- Modify: `crates/saan_core/src/shaver/sql.rs`
- Modify: `crates/saan_core/src/shaver/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `crates/saan_core/src/shaver/sql.rs`:

```rust
#[test]
fn postgres_dialect_parses_double_colon_cast() {
    // PostgreSQL-specific :: cast syntax — GenericDialect rejects this
    let shaver = SqlShaver::with_dialect(SqlDialect::Postgres);
    let mut f = NamedTempFile::with_suffix(".sql").unwrap();
    f.write_all(b"CREATE TABLE t AS SELECT id::text FROM src").unwrap();
    let strand = shaver.shave(f.path()).unwrap();
    assert!(strand.nodes.iter().any(|n| n.id == "t"));
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test -p saan_core -- postgres_dialect_parses_double_colon_cast --nocapture
```

Expected: FAIL — `SqlShaver::with_dialect` and `SqlDialect` do not exist yet.

- [ ] **Step 3: Add `SqlDialect` enum and update `SqlShaver` struct**

In `crates/saan_core/src/shaver/sql.rs`:

Replace the imports block (lines 1–13) with:

```rust
use std::collections::HashMap;
use std::path::Path;

use sqlparser::ast::{
    Ident, ObjectName, Query, Select, SetExpr, Statement, TableFactor, TableWithJoins,
};
use sqlparser::dialect::{
    AnsiDialect, BigQueryDialect, ClickHouseDialect, DuckDbDialect, GenericDialect,
    HiveDialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect, RedshiftDialect,
    SnowflakeDialect, SQLiteDialect,
};
use sqlparser::parser::Parser;

use crate::graph::{Edge, Node};
use crate::shaver::{Shaver, ShaverError};
use crate::strand::Strand;

#[derive(Debug, Clone, Default)]
pub enum SqlDialect {
    #[default]
    Generic,
    Ansi,
    Postgres,
    MySql,
    MsSql,
    BigQuery,
    Snowflake,
    Hive,
    Redshift,
    SQLite,
    DuckDb,
    ClickHouse,
}
```

Replace `pub struct SqlShaver;` with:

```rust
pub struct SqlShaver {
    dialect: SqlDialect,
}

impl SqlShaver {
    pub fn new() -> Self {
        Self { dialect: SqlDialect::Generic }
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self { dialect }
    }
}

impl Default for SqlShaver {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Update `shave()` to dispatch on dialect**

In the `shave()` method, replace the single `Parser::parse_sql(&GenericDialect {}, &sql).map_err(...)` call with:

```rust
let statements = {
    let parse = |d: &dyn sqlparser::dialect::Dialect| {
        Parser::parse_sql(d, &sql).map_err(|e| ShaverError::Parse {
            path: input.to_path_buf(),
            message: e.to_string(),
        })
    };
    match &self.dialect {
        SqlDialect::Generic    => parse(&GenericDialect {}),
        SqlDialect::Ansi       => parse(&AnsiDialect {}),
        SqlDialect::Postgres   => parse(&PostgreSqlDialect {}),
        SqlDialect::MySql      => parse(&MySqlDialect {}),
        SqlDialect::MsSql      => parse(&MsSqlDialect {}),
        SqlDialect::BigQuery   => parse(&BigQueryDialect {}),
        SqlDialect::Snowflake  => parse(&SnowflakeDialect {}),
        SqlDialect::Hive       => parse(&HiveDialect {}),
        SqlDialect::Redshift   => parse(&RedshiftDialect {}),
        SqlDialect::SQLite     => parse(&SQLiteDialect {}),
        SqlDialect::DuckDb     => parse(&DuckDbDialect {}),
        SqlDialect::ClickHouse => parse(&ClickHouseDialect {}),
    }?
};
```

- [ ] **Step 5: Fix the `shave_sql` test helper**

`SqlShaver` is no longer a unit struct, so `SqlShaver.shave(...)` won't compile. Change the helper in the `tests` module:

```rust
fn shave_sql(sql: &str) -> Strand {
    let mut f = NamedTempFile::with_suffix(".sql").unwrap();
    f.write_all(sql.as_bytes()).unwrap();
    SqlShaver::new().shave(f.path()).unwrap()
}
```

- [ ] **Step 6: Update `with_builtins()` and add `with_sql_dialect()` in `shaver/mod.rs`**

Change line 43 from `Arc::new(sql::SqlShaver)` to `Arc::new(sql::SqlShaver::new())`:

```rust
pub fn with_builtins() -> Self {
    let mut r = Self::new();
    r.register(Arc::new(sql::SqlShaver::new()));
    r
}
```

Add `with_sql_dialect()` directly after `with_builtins()`:

```rust
pub fn with_sql_dialect(dialect: sql::SqlDialect) -> Self {
    let mut r = Self::new();
    r.register(Arc::new(sql::SqlShaver::with_dialect(dialect)));
    r
}
```

- [ ] **Step 7: Run all saan_core tests**

```
cargo test -p saan_core
```

Expected: all existing tests pass + `postgres_dialect_parses_double_colon_cast` passes.

- [ ] **Step 8: Commit**

```
git add crates/saan_core/src/shaver/sql.rs crates/saan_core/src/shaver/mod.rs
git commit -m "feat(core): add SqlDialect enum and per-dialect SqlShaver config"
```

---

## Task 2: CLI `--dialect` flag for `saan prepare`

**Files:**
- Modify: `crates/saan_core/src/lib.rs`
- Modify: `crates/saan_cli/src/main.rs`
- Modify: `crates/saan_cli/src/commands/prepare.rs`
- Modify: `crates/saan_cli/tests/cli_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Add to `crates/saan_cli/tests/cli_integration.rs`:

```rust
#[test]
fn prepare_with_postgres_dialect_parses_cast_syntax() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("cast.sql");

    std::fs::write(&sql_path, b"CREATE TABLE t AS SELECT id::text FROM src").unwrap();

    saan().arg("init").arg(dir.path()).assert().success();

    saan()
        .arg("prepare")
        .arg(dir.path())
        .arg("--store").arg(&store_path)
        .arg("--dialect").arg("postgres")
        .assert()
        .success();
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test -p saan_cli -- prepare_with_postgres_dialect_parses_cast_syntax --nocapture
```

Expected: FAIL — `--dialect` flag is unknown.

- [ ] **Step 3: Export `SqlDialect` from `saan_core/src/lib.rs`**

Add to `lib.rs`:

```rust
pub use shaver::sql::SqlDialect;
```

- [ ] **Step 4: Add `CliDialect` enum and `From` impl in `main.rs`**

Add after the existing `use` statements at the top of `main.rs`:

```rust
use saan_core::SqlDialect;

#[derive(Debug, Clone, clap::ValueEnum)]
enum CliDialect {
    Generic,
    Ansi,
    Postgres,
    Mysql,
    Mssql,
    Bigquery,
    Snowflake,
    Hive,
    Redshift,
    Sqlite,
    Duckdb,
    Clickhouse,
}

impl From<CliDialect> for SqlDialect {
    fn from(d: CliDialect) -> Self {
        match d {
            CliDialect::Generic    => SqlDialect::Generic,
            CliDialect::Ansi       => SqlDialect::Ansi,
            CliDialect::Postgres   => SqlDialect::Postgres,
            CliDialect::Mysql      => SqlDialect::MySql,
            CliDialect::Mssql      => SqlDialect::MsSql,
            CliDialect::Bigquery   => SqlDialect::BigQuery,
            CliDialect::Snowflake  => SqlDialect::Snowflake,
            CliDialect::Hive       => SqlDialect::Hive,
            CliDialect::Redshift   => SqlDialect::Redshift,
            CliDialect::Sqlite     => SqlDialect::SQLite,
            CliDialect::Duckdb     => SqlDialect::DuckDb,
            CliDialect::Clickhouse => SqlDialect::ClickHouse,
        }
    }
}
```

- [ ] **Step 5: Update the `Prepare` variant in `main.rs`**

Replace the `Prepare` variant with:

```rust
/// Extract metadata from source files into the staging tables
Prepare {
    /// Input directory or file to walk
    input: PathBuf,
    /// Path to the .saan store
    #[arg(long, default_value = ".saan")]
    store: PathBuf,
    /// SQL dialect for parsing
    #[arg(long, value_enum, default_value = "generic")]
    dialect: CliDialect,
},
```

Update the `Prepare` dispatch arm:

```rust
Commands::Prepare { input, store, dialect } => {
    commands::prepare::run(&input, &store, dialect.into())?
}
```

- [ ] **Step 6: Update `prepare.rs` to accept dialect**

Replace `crates/saan_cli/src/commands/prepare.rs` in full:

```rust
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
    let registry = ShaverRegistry::with_sql_dialect(dialect);
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
```

- [ ] **Step 7: Run all workspace tests**

```
cargo test --workspace
```

Expected: all tests pass including the new dialect test.

- [ ] **Step 8: Commit**

```
git add crates/saan_core/src/lib.rs crates/saan_cli/src/main.rs crates/saan_cli/src/commands/prepare.rs crates/saan_cli/tests/cli_integration.rs
git commit -m "feat(cli): add --dialect flag to saan prepare"
```

---

## Task 3: `Store::inspect()` — library layer

**Files:**
- Modify: `crates/saan_core/src/store.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/saan_core/src/store.rs`:

```rust
#[test]
fn inspect_detects_orphan_node() {
    let (store, _dir) = make_store();
    store
        .write_strands_to_staging(&[strand_with(&[("orphan", "Orphan")], &[])])
        .unwrap();
    store.apply_staging().unwrap();

    let report = store.inspect().unwrap();
    assert!(report.orphan_nodes.contains(&"orphan".to_string()));
    assert!(!report.cycle_detected);
    assert!(report.external_refs.is_empty());
}

#[test]
fn inspect_reports_external_ref() {
    let (store, _dir) = make_store();
    // raw.orders used as a source edge endpoint but has no node row
    store
        .write_strands_to_staging(&[strand_with(
            &[("stg.orders", "Staged")],
            &[("raw.orders", "stg.orders")],
        )])
        .unwrap();
    store.apply_staging().unwrap();

    let report = store.inspect().unwrap();
    assert!(report.external_refs.contains(&"raw.orders".to_string()));
}

#[test]
fn inspect_clean_graph_has_no_issues() {
    let (store, _dir) = make_store();
    store
        .write_strands_to_staging(&[strand_with(
            &[("raw.orders", "Raw"), ("stg.orders", "Staged")],
            &[("raw.orders", "stg.orders")],
        )])
        .unwrap();
    store.apply_staging().unwrap();

    let report = store.inspect().unwrap();
    assert!(report.orphan_nodes.is_empty());
    assert!(report.external_refs.is_empty());
    assert!(!report.cycle_detected);
    assert_eq!(report.total_nodes, 2);
    assert_eq!(report.total_edges, 1);
}
```

- [ ] **Step 2: Run to verify they fail**

```
cargo test -p saan_core -- inspect --nocapture
```

Expected: FAIL — `InspectReport` and `Store::inspect` do not exist.

- [ ] **Step 3: Add `InspectReport` to `store.rs`**

Add after the `StoreError` definition (before `pub struct Store`):

```rust
pub struct InspectReport {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub orphan_nodes: Vec<String>,
    pub cycle_detected: bool,
    pub external_refs: Vec<String>,
}
```

- [ ] **Step 4: Implement `Store::inspect()`**

Add to `impl Store`, after `load_graph()`:

```rust
pub fn inspect(&self) -> Result<InspectReport, StoreError> {
    let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM nodes")?;
    let total_nodes: usize = stmt
        .query_row([], |row| row.get::<_, i64>(0))
        .map(|n| n as usize)?;

    let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM edges")?;
    let total_edges: usize = stmt
        .query_row([], |row| row.get::<_, i64>(0))
        .map(|n| n as usize)?;

    let mut stmt = self.conn.prepare(
        "SELECT id FROM nodes
         WHERE id NOT IN (SELECT from_id FROM edges)
           AND id NOT IN (SELECT to_id   FROM edges)
         ORDER BY id",
    )?;
    let orphan_nodes: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = self.conn.prepare(
        "SELECT DISTINCT id FROM (
             SELECT from_id AS id FROM edges
              WHERE from_id NOT IN (SELECT id FROM nodes)
             UNION ALL
             SELECT to_id AS id FROM edges
              WHERE to_id NOT IN (SELECT id FROM nodes)
         ) t ORDER BY id",
    )?;
    let external_refs: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let graph = self.load_graph()?;
    let cycle_detected = graph.has_cycle();

    Ok(InspectReport {
        total_nodes,
        total_edges,
        orphan_nodes,
        cycle_detected,
        external_refs,
    })
}
```

- [ ] **Step 5: Run the three inspect tests**

```
cargo test -p saan_core -- inspect --nocapture
```

Expected: all three tests pass.

- [ ] **Step 6: Export `InspectReport` from `lib.rs`**

Add to `crates/saan_core/src/lib.rs`:

```rust
pub use store::InspectReport;
```

- [ ] **Step 7: Run workspace tests**

```
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```
git add crates/saan_core/src/store.rs crates/saan_core/src/lib.rs
git commit -m "feat(core): add Store::inspect() with orphan, cycle, and external-ref detection"
```

---

## Task 4: `saan inspect` CLI command

**Files:**
- Create: `crates/saan_cli/src/commands/inspect.rs`
- Modify: `crates/saan_cli/src/commands/mod.rs`
- Modify: `crates/saan_cli/src/main.rs`
- Modify: `crates/saan_cli/tests/cli_integration.rs`

- [ ] **Step 1: Write the failing integration test (and remove the old stub test)**

In `crates/saan_cli/tests/cli_integration.rs`:

Remove the existing test:
```rust
#[test]
fn inspect_exits_nonzero_with_not_implemented() {
    saan()
        .arg("inspect")
        .assert()
        .failure()
        .stderr(contains("not implemented"));
}
```

Replace it with:

```rust
#[test]
fn inspect_reports_node_and_edge_counts() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("inspect")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("Nodes:"))
        .stdout(contains("Edges:"));
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test -p saan_cli -- inspect_reports_node_and_edge_counts --nocapture
```

Expected: FAIL — inspect exits non-zero (still the Phase 1 stub).

- [ ] **Step 3: Create `inspect.rs`**

Create `crates/saan_cli/src/commands/inspect.rs`:

```rust
use anyhow::Result;
use saan_core::Store;
use std::path::Path;

pub fn run(store_path: &Path) -> Result<()> {
    if !store_path.exists() {
        anyhow::bail!(
            "Store not found at '{}'. Run `saan init` first.",
            store_path.display()
        );
    }
    let store = Store::open(store_path)?;
    let report = store.inspect()?;

    println!("=== saan inspect ===");
    println!("Nodes:  {}", report.total_nodes);
    println!("Edges:  {}", report.total_edges);
    println!();

    if report.cycle_detected {
        println!("WARNING: cycle detected in graph");
    }

    println!(
        "Orphan nodes  ({}): {}",
        report.orphan_nodes.len(),
        if report.orphan_nodes.is_empty() {
            "none".to_string()
        } else {
            report.orphan_nodes.join(", ")
        }
    );
    println!(
        "External refs ({}): {}",
        report.external_refs.len(),
        if report.external_refs.is_empty() {
            "none".to_string()
        } else {
            report.external_refs.join(", ")
        }
    );

    Ok(())
}
```

- [ ] **Step 4: Add to `mod.rs`**

Add to `crates/saan_cli/src/commands/mod.rs`:

```rust
pub mod inspect;
```

- [ ] **Step 5: Update `main.rs` — update `Inspect` variant and dispatch**

Replace the `Inspect` variant (currently a unit variant):

```rust
/// Validate the graph structure
Inspect {
    /// Path to the .saan store
    #[arg(long, default_value = ".saan")]
    store: PathBuf,
},
```

Update the combined dispatch arm. The current arm is:
```rust
Commands::Interlace | Commands::Inspect | Commands::View => {
    eprintln!("not implemented in Phase 1");
    std::process::exit(1);
}
```

Replace it with three separate arms (keep `Interlace` and `View` stubbed for now):

```rust
Commands::Interlace => {
    eprintln!("not implemented in Phase 1");
    std::process::exit(1);
}
Commands::Inspect { store } => commands::inspect::run(&store)?,
Commands::View => {
    eprintln!("not implemented in Phase 1");
    std::process::exit(1);
}
```

- [ ] **Step 6: Run all workspace tests**

```
cargo test --workspace
```

Expected: all tests pass including `inspect_reports_node_and_edge_counts`.

- [ ] **Step 7: Commit**

```
git add crates/saan_cli/src/commands/inspect.rs crates/saan_cli/src/commands/mod.rs crates/saan_cli/src/main.rs crates/saan_cli/tests/cli_integration.rs
git commit -m "feat(cli): implement saan inspect command"
```

---

## Task 5: `Store::interlace_staging()` — library layer

**Files:**
- Modify: `crates/saan_core/src/store.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `store.rs`:

```rust
#[test]
fn interlace_adds_transitive_edge() {
    let (store, _dir) = make_store();
    // Staging: a → b → c; interlace should add a → c
    store
        .write_strands_to_staging(&[strand_with(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        )])
        .unwrap();

    let added = store.interlace_staging().unwrap();
    assert_eq!(added, 1);

    store.apply_staging().unwrap();
    let g = store.load_graph().unwrap();
    assert_eq!(g.edge_count(), 3); // a→b, b→c, a→c
}

#[test]
fn interlace_single_hop_adds_nothing() {
    let (store, _dir) = make_store();
    store
        .write_strands_to_staging(&[strand_with(&[("a", "A"), ("b", "B")], &[("a", "b")])])
        .unwrap();

    let added = store.interlace_staging().unwrap();
    assert_eq!(added, 0);
}

#[test]
fn interlace_empty_staging_is_noop() {
    let (store, _dir) = make_store();
    assert_eq!(store.interlace_staging().unwrap(), 0);
}

#[test]
fn interlace_is_idempotent() {
    let (store, _dir) = make_store();
    store
        .write_strands_to_staging(&[strand_with(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        )])
        .unwrap();

    let first = store.interlace_staging().unwrap();
    // staging still contains all three edges (direct + computed); calling again adds nothing
    let second = store.interlace_staging().unwrap();
    assert_eq!(first, 1);
    assert_eq!(second, 0, "second call must not add duplicate edges");
}
```

- [ ] **Step 2: Run to verify they fail**

```
cargo test -p saan_core -- interlace --nocapture
```

Expected: FAIL — `Store::interlace_staging` does not exist.

- [ ] **Step 3: Add import and implement `interlace_staging()`**

Add this import at the top of `store.rs` (alongside the existing `use std::path::Path`):

```rust
use std::collections::{HashMap, HashSet, VecDeque};
```

Add to `impl Store`, before `load_graph()`:

```rust
pub fn interlace_staging(&self) -> Result<usize, StoreError> {
    let mut stmt = self.conn.prepare("SELECT from_id, to_id FROM staging_edges")?;
    let pairs: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    if pairs.is_empty() {
        return Ok(0);
    }

    let existing: HashSet<(String, String)> = pairs.iter().cloned().collect();

    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in &pairs {
        adj.entry(from.clone()).or_default().push(to.clone());
    }

    let sources: Vec<String> = adj.keys().cloned().collect();
    let mut new_edges: HashSet<(String, String)> = HashSet::new();

    for start in &sources {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        visited.insert(start.clone());
        queue.push_back(start.clone());

        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&current) {
                for neighbor in neighbors {
                    let pair = (start.clone(), neighbor.clone());
                    if neighbor != start && !existing.contains(&pair) {
                        new_edges.insert(pair);
                    }
                    if visited.insert(neighbor.clone()) {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
    }

    let count = new_edges.len();
    if count == 0 {
        return Ok(0);
    }

    self.conn.execute_batch("BEGIN")?;
    let result: Result<(), StoreError> = (|| {
        let mut stmt = self.conn.prepare(
            "INSERT INTO staging_edges (from_id, to_id, source_path) VALUES (?, ?, ?)",
        )?;
        for (from, to) in &new_edges {
            stmt.execute(duckdb::params![from, to, "<interlaced>"])?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = self.conn.execute_batch("ROLLBACK");
    } else {
        self.conn.execute_batch("COMMIT")?;
    }
    result?;

    Ok(count)
}
```

- [ ] **Step 4: Run the four interlace tests**

```
cargo test -p saan_core -- interlace --nocapture
```

Expected: all four tests pass.

- [ ] **Step 5: Run workspace tests**

```
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```
git add crates/saan_core/src/store.rs
git commit -m "feat(core): add Store::interlace_staging() for transitive edge computation"
```

---

## Task 6: `saan interlace` CLI command

**Files:**
- Create: `crates/saan_cli/src/commands/interlace.rs`
- Modify: `crates/saan_cli/src/commands/mod.rs`
- Modify: `crates/saan_cli/src/main.rs`
- Modify: `crates/saan_cli/tests/cli_integration.rs`

- [ ] **Step 1: Write the failing integration test (and remove the old stub test)**

In `crates/saan_cli/tests/cli_integration.rs`:

Remove the existing test:
```rust
#[test]
fn interlace_exits_nonzero_with_not_implemented() {
    saan()
        .arg("interlace")
        .assert()
        .failure()
        .stderr(contains("not implemented"));
}
```

Replace it with:

```rust
#[test]
fn interlace_adds_transitive_edge_end_to_end() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    // a → b → c; interlace should compute a → c
    std::fs::write(
        &sql_path,
        b"CREATE TABLE b AS SELECT * FROM a;\nCREATE TABLE c AS SELECT * FROM b;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("interlace")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("1 computed edge"));

    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("3 edge(s)"));
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test -p saan_cli -- interlace_adds_transitive_edge_end_to_end --nocapture
```

Expected: FAIL — interlace exits non-zero (Phase 1 stub).

- [ ] **Step 3: Create `interlace.rs`**

Create `crates/saan_cli/src/commands/interlace.rs`:

```rust
use anyhow::Result;
use saan_core::Store;
use std::path::Path;

pub fn run(store_path: &Path) -> Result<()> {
    if !store_path.exists() {
        anyhow::bail!(
            "Store not found at '{}'. Run `saan init` first.",
            store_path.display()
        );
    }
    let store = Store::open(store_path)?;
    let added = store.interlace_staging()?;
    println!("Interlaced: {} computed edge(s) added to staging.", added);
    Ok(())
}
```

- [ ] **Step 4: Add to `mod.rs`**

Add to `crates/saan_cli/src/commands/mod.rs`:

```rust
pub mod interlace;
```

- [ ] **Step 5: Update `main.rs` — update `Interlace` variant and dispatch**

Replace the `Interlace` unit variant:

```rust
/// Compute derived lineage edges in staging
Interlace {
    /// Path to the .saan store
    #[arg(long, default_value = ".saan")]
    store: PathBuf,
},
```

Update the dispatch arm (the `Interlace` stub set in Task 4 Step 5):

```rust
Commands::Interlace { store } => commands::interlace::run(&store)?,
```

- [ ] **Step 6: Run all workspace tests**

```
cargo test --workspace
```

Expected: all tests pass including `interlace_adds_transitive_edge_end_to_end`.

- [ ] **Step 7: Commit**

```
git add crates/saan_cli/src/commands/interlace.rs crates/saan_cli/src/commands/mod.rs crates/saan_cli/src/main.rs crates/saan_cli/tests/cli_integration.rs
git commit -m "feat(cli): implement saan interlace command"
```

---

## Self-Review

### Spec coverage

| ROADMAP Phase 2 requirement | Covered by |
|-----------------------------|------------|
| `saan interlace` automated edge-building | Tasks 5–6: BFS transitive closure on staging edges |
| `saan inspect` orphan nodes | Task 3–4: `orphan_nodes` SQL query |
| `saan inspect` cycles | Tasks 3–4: `has_cycle()` via petgraph |
| `saan inspect` edges to unknown nodes | Tasks 3–4: `external_refs` SQL query |
| `saan inspect` schema drift | **Deferred** — no clear spec for "drift threshold"; `last_seen_at` is in place for Phase 3+ to build on |
| Per-Shaver dialect configuration | Tasks 1–2: `SqlDialect` enum, `--dialect` flag |

### Placeholder check

None — every step contains concrete code, exact commands, and expected output.

### Type consistency check

- `Edge.from` / `Edge.to` — verified from `graph.rs:27–30`
- `Node::new(id, label, source_type)` — verified from `graph.rs:13`
- `Edge::new(from, to)` — verified from `graph.rs:33`
- `strand_with` helper in store tests uses `Edge::new(*from, *to)` — matches graph.rs
- `ShaverRegistry::with_sql_dialect(dialect)` added in Task 1; used in Task 2's `prepare.rs`
- `Store::inspect()` added in Task 3; used in Task 4's `inspect.rs`
- `Store::interlace_staging()` added in Task 5; used in Task 6's `interlace.rs`
- CLI integration tests use `assert_cmd::Command` pattern matching existing tests in `cli_integration.rs`

---

# Implementation Plan — Phase 3 Visualization

**Status:** Pending
**Scope:** `saan view` command — static HTML+SVG output first, then the full WASM interactive visualizer via `saan_mesh`.

---

## 1. Goal

After a full `prepare -> interlace -> apply` pipeline a user can run:

```powershell
saan_cli view --store my-pipeline/.saan --out lineage.html
# Opens lineage.html in a browser: interactive SVG graph, clickable nodes
```

Phase 3 ships in two milestones:

- **3A - Static HTML+SVG** - `saan view` writes a self-contained HTML file with an inline force-directed SVG. No WASM, no server.
- **3B - WASM React** - `saan_mesh` is compiled to WASM and wired into a React app that embeds in the same HTML file. Replaces the pure-SVG renderer.

---

## 2. Out of Scope

- Server-side rendering / hot reload
- Authentication or multi-user sharing
- Export to PNG/PDF (future)
- WASM streaming / code splitting (Phase 6 performance work)

---

## 3. Crate-Level Changes

### 3.1 `saan_core` - SVG renderer

New module `saan_core/src/render.rs`:

```
pub struct SvgRenderer { /* layout settings */ }
impl SvgRenderer {
    pub fn render(graph: &Graph, config: &RenderConfig) -> String  // returns full SVG string
}
```

Uses a simple force-directed layout (spring model, Rust-only, no JS). The SVG is self-contained with inline `<style>` and `<script>` for pan/zoom via `viewBox` manipulation.

### 3.2 `saan_core` - HTML wrapper

New `saan_core/src/html.rs`:

```
pub fn wrap_svg_in_html(svg: &str, title: &str) -> String  // returns full HTML5 document string
```

### 3.3 `saan_cli` - `saan view`

New `crates/saan_cli/src/commands/view.rs`:

```
pub fn run(store_path: &Path, out_path: &Path) -> Result<()>
```

- Opens store, loads graph, calls `SvgRenderer::render` + `wrap_svg_in_html`
- Writes file to `out_path` (default: `lineage.html` in current directory)

Update `main.rs` - replace `View` unit variant with:

```rust
View {
    #[arg(long, default_value = ".saan")]
    store: PathBuf,
    #[arg(long, default_value = "lineage.html")]
    out: PathBuf,
},
```

### 3.4 `saan_mesh` - WASM milestone (3B)

Add dependencies to `crates/saan_mesh/Cargo.toml`:

```toml
wasm-bindgen = "0.2"
saan_core = { path = "../saan_core" }
js-sys = "0.3"
web-sys = { version = "0.3", features = ["Window", "Document", "Element", "SvgElement"] }
```

New exported WASM function in `crates/saan_mesh/src/lib.rs`:

```rust
#[wasm_bindgen]
pub fn render_graph(nodes_json: &str, edges_json: &str, config_json: &str) -> String
```

Add `web/` directory at workspace root - React + TypeScript app:

```
web/
  package.json
  src/
    App.tsx
    components/
      GraphView.tsx
```

---

## 4. Task Breakdown

### Task 1: `SvgRenderer` and `wrap_svg_in_html` (library)

**Files:**
- Create: `crates/saan_core/src/render.rs`
- Create: `crates/saan_core/src/html.rs`
- Modify: `crates/saan_core/src/lib.rs`

Tests:
- Empty graph produces valid SVG (well-formed, no panics)
- Single-node graph produces one `<circle>` element
- Edge produces one `<line>` or `<path>` element
- HTML wrapper contains `<!DOCTYPE html>` and the SVG string

### Task 2: `saan view` CLI - static milestone (3A)

**Files:**
- Create: `crates/saan_cli/src/commands/view.rs`
- Modify: `crates/saan_cli/src/commands/mod.rs`
- Modify: `crates/saan_cli/src/main.rs`
- Modify: `crates/saan_cli/tests/cli_integration.rs`

Replace the "not implemented" stub. Integration test: full pipeline -> `saan view` -> output file exists and contains `<svg`.

### Task 3: `saan_mesh` WASM build (milestone 3B)

**Files:**
- Modify: `crates/saan_mesh/Cargo.toml`
- Modify: `crates/saan_mesh/src/lib.rs`
- Create: `crates/saan_mesh/.cargo/config.toml` (target = wasm32-unknown-unknown)
- Create: `web/package.json`, `web/src/App.tsx`, `web/src/components/GraphView.tsx`

Deliverable: `wasm-pack build crates/saan_mesh --target web` succeeds. The React app renders a graph from WASM.

### Task 4: `saan view` - embed WASM bundle (milestone 3B)

Update `commands/view.rs` to embed the compiled WASM+JS bundle inline in the HTML when the `--wasm` flag is passed. Fall back to pure SVG if WASM bundle is not present at build time.

---

## 5. Success Criteria

1. `saan view` exits zero and writes a valid HTML file for any non-empty graph.
2. Opening the HTML file in a browser shows labeled nodes and directed edges.
3. `cargo test --workspace` still passes with all prior tests.
4. `wasm-pack build` succeeds for `saan_mesh` (milestone 3B).

---

# Implementation Plan — Phase 4 Python SDK

**Status:** Complete
**Branch:** `feat/phase-4-5` (PR #5)
**Scope:** PyO3 bindings via `maturin` exposing the full `saan_core` surface as a Python package `saan_ops`.

---

## 1. Goal

```python
import saan_ops

conn = saan_ops.connect("my-pipeline/.saan")
conn.prepare("my-pipeline/", dialect="postgres")
conn.interlace()
conn.apply()
report = conn.inspect()
print(report.total_nodes, report.total_edges, report.orphan_nodes)
```

---

## 2. Out of Scope

- Async Python API (`asyncio`) - deferred, covered by Phase 6 AsyncShaver
- Pandas/Polars/Arrow integration - Phase 5 (depends on query layer)
- PyPI publishing pipeline - Phase 6 deployment work

---

## 3. Crate-Level Changes

### 3.1 New crate `saan_py`

Add to workspace:

```
crates/saan_py/
  Cargo.toml      - pyo3 + maturin, depends on saan_core
  pyproject.toml  - maturin build system config
  src/
    lib.rs        - #[pymodule] fn saan_ops
    connection.rs - #[pyclass] SaanConnection
    report.rs     - #[pyclass] PyInspectReport
    graph.rs      - #[pyclass] PyGraph
```

`Cargo.toml` dependencies:

```toml
[dependencies]
pyo3 = { version = "0.23", features = ["extension-module"] }
saan_core = { path = "../saan_core" }

[lib]
name = "saan_ops"
crate-type = ["cdylib"]
```

### 3.2 Python package layout

```
python/
  saan_ops/
    __init__.py   - re-exports connect(), __version__
  tests/
    test_connect.py
    test_pipeline.py
    test_inspect.py
```

---

## 4. Task Breakdown

### Task 1: Workspace scaffold and `import saan_ops`

**Files:**
- Create: `crates/saan_py/Cargo.toml`, `pyproject.toml`, `src/lib.rs`
- Modify: `Cargo.toml` (add `crates/saan_py` to `members`)
- Create: `python/saan_ops/__init__.py`

Deliverable: `maturin develop` installs the module. `import saan_ops; saan_ops.__version__` works.

### Task 2: `SaanConnection` - init and prepare

**Files:**
- Create: `crates/saan_py/src/connection.rs`

```python
conn = saan_ops.connect(path)               # opens Store, init_schema if new
conn.prepare(input, dialect="generic")
```

Tests: connect on new path creates `.saan`; prepare stages nodes from a SQL fixture.

### Task 3: `apply`, `interlace`, `inspect`

**Files:**
- Modify: `crates/saan_py/src/connection.rs`
- Create: `crates/saan_py/src/report.rs`

```python
conn.interlace()
conn.apply()
report = conn.inspect()   # -> PyInspectReport
```

Tests: full pipeline test mirrors the CLI end-to-end test.

### Task 4: `graph()` accessor and `PyGraph`

**Files:**
- Create: `crates/saan_py/src/graph.rs`

```python
g = conn.graph()
g.node_count()    # int
g.edge_count()    # int
g.nodes()         # list[dict]
g.edges()         # list[dict]
g.has_cycle()     # bool
```

---

## 5. Success Criteria

1. `maturin develop` succeeds.
2. `python -m pytest python/tests/` - all tests pass.
3. Full lineage pipeline runnable in a Python script with no CLI subprocess.
4. `cargo test --workspace` still passes.

---

# Implementation Plan — Phase 5 Ad-Hoc Query

**Status:** Complete
**Branch:** `feat/phase-4-5` (PR #5)
**Scope:** SQL passthrough on the CLI (`saan query`) and Python SDK (`.query().to_pandas()` etc.). No new ingestion path - queries read directly from the DuckDB store that lineage writes to.

---

## 1. Goal

```powershell
saan_cli query "SELECT id, label FROM nodes WHERE source_type = 'sql'" --store my-pipeline/.saan
```

```python
df = conn.query("SELECT from_id, to_id FROM edges").to_pandas()
```

---

## 2. Out of Scope

- Write-path SQL (INSERT/UPDATE via query) - the query surface is read-only in Phase 5
- Result caching
- Named saved queries / views

---

## 3. Crate-Level Changes

### 3.1 `saan_core` - `Store::query()`

Add to `Store` in `store.rs`:

```rust
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,  // all values rendered to String
}

impl Store {
    pub fn query(&self, sql: &str) -> Result<QueryResult, StoreError>
}
```

### 3.2 `saan_cli` - `saan query`

New `crates/saan_cli/src/commands/query.rs`:

```rust
pub fn run(sql: &str, store_path: &Path, format: OutputFormat) -> Result<()>
```

`OutputFormat`: `Table` (default, ASCII grid) | `Csv` | `Json`.

Update `main.rs`:

```rust
Query {
    sql: String,
    #[arg(long, default_value = ".saan")]
    store: PathBuf,
    #[arg(long, value_enum, default_value = "table")]
    format: OutputFormat,
},
```

### 3.3 `saan_py` - `.query()` and result converters

```python
result = conn.query(sql)
result.columns              # list[str]
result.rows                 # list[list[str]]
result.to_pandas()          # pandas.DataFrame
result.to_polars()          # polars.DataFrame
result.to_arrow()           # pyarrow.Table
```

Optional deps in `pyproject.toml` extras: `pandas`, `polars`, `arrow`.

---

## 4. Task Breakdown

### Task 1: `Store::query()` and `QueryResult` (library)

**Files:**
- Modify: `crates/saan_core/src/store.rs`
- Modify: `crates/saan_core/src/lib.rs`

Tests:
- `SELECT COUNT(*) FROM nodes` on populated store returns `[["2"]]`
- Empty store returns zero rows with correct column name
- Invalid SQL returns `Err(StoreError::Db(...))`

### Task 2: `saan query` CLI command

**Files:**
- Create: `crates/saan_cli/src/commands/query.rs`
- Modify: `crates/saan_cli/src/commands/mod.rs`, `src/main.rs`, `tests/cli_integration.rs`

Integration tests:
- `saan query "SELECT COUNT(*) FROM nodes"` after pipeline exits zero and prints a row
- `--format csv` outputs comma-separated header + rows
- `--format json` outputs a JSON array of objects
- Invalid SQL exits non-zero with error on stderr

### Task 3: Python `.query()` binding

**Files:**
- Modify: `crates/saan_py/src/connection.rs`
- Create: `crates/saan_py/src/query_result.rs`

Tests:
- `conn.query("SELECT 1").rows == [["1"]]`
- `.to_pandas()` returns a DataFrame with correct column names

---

## 5. Success Criteria

1. `saan query "SELECT * FROM nodes"` exits zero and prints a table.
2. `--format csv` and `--format json` produce parseable output.
3. `conn.query(sql).to_pandas()` returns a DataFrame with correct schema.
4. `cargo test --workspace` and `python -m pytest python/tests/` both pass.

---

# Implementation Plan — Phase 6 Ecosystem and Additional Shavers

**Status:** Pending
**Scope:** Expand the Shaver ecosystem, add an `AsyncShaver` trait, a runtime plugin system, performance work, and deployment artifacts.

---

## 1. Goal

- A dbt user can run `saan prepare manifest.json` and get a lineage graph from their dbt manifest.
- An external team can publish a crate that adds a new file type without modifying `saan_core`.
- `saan prepare` on a 10k-file repository completes in under 10 seconds via parallel ingestion.

---

## 2. Task Breakdown

### Task 1: `DbtShaver` - dbt manifest parser

**Files:**
- Create: `crates/saan_core/src/shaver/dbt.rs`
- Modify: `crates/saan_core/src/shaver/mod.rs`

Parses `manifest.json` produced by `dbt compile`/`dbt run`. Extracts `nodes`, `sources`, and `parent_map` to build one `Strand` per model. Handles `ref()` as an explicit edge between dbt model nodes.

Tests: fixture `tests/fixtures/dbt/manifest_v10.json` - verifies node count and edge direction; `ref()` edges point correctly from upstream to downstream.

### Task 2: `JsonShaver` / `YamlShaver` / `TomlShaver`

**Files:**
- Create: `crates/saan_core/src/shaver/manifest.rs`
- Modify: `crates/saan_core/src/shaver/mod.rs`

Reads manifest files following `{nodes: [...], edges: [...]}` schema or a user-defined mapping via `--mapping` config. Primary use: custom pipeline metadata files.

### Task 3: `AsyncShaver` trait

**Files:**
- Create: `crates/saan_core/src/shaver/async_shaver.rs`
- Modify: `crates/saan_core/Cargo.toml` (add `tokio` as optional dep, feature-gate `async`)

```rust
#[async_trait]
pub trait AsyncShaver: Send + Sync {
    async fn shave_async(&self, path: &Path) -> Result<Strand, ShaverError>;
}
```

`ShaverRegistry` gets `async fn shave_path_async` that drives a `tokio::task::spawn` pool.

Initial implementations: HTTP-fetched SQL (Snowflake `SHOW TABLES`, BigQuery table metadata via REST).

### Task 4: Plugin system

**Files:**
- Create: `crates/saan_core/src/plugin.rs`
- Modify: `crates/saan_core/Cargo.toml` (add `libloading`)

```rust
pub trait ShaverPlugin: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn shaver(&self) -> Arc<dyn Shaver>;
}
```

`ShaverRegistry::load_plugin(path: &Path)` opens a `cdylib` via `libloading` and calls a `#[no_mangle] fn saan_plugin_register() -> Box<dyn ShaverPlugin>`.

CLI: `saan prepare --plugin ./my_shaver.dll input/`

### Task 5: Parallel ingestion

**Files:**
- Modify: `crates/saan_core/src/shaver/mod.rs`
- Modify: `crates/saan_core/Cargo.toml` (add `rayon = "1"`)

Replace the serial `shave_path` walk with a `rayon::par_iter` over files. Final `write_strands_to_staging` remains a single serial batch write.

Benchmark: `cargo bench` with a 1000-file fixture. Target: >= 4x speedup on 4-core hardware.

### Task 6: Deployment artifacts

**Files:**
- Create: `Dockerfile`
- Create: `.github/workflows/release.yml`
- Create: `docker-compose.yml`

`Dockerfile`: multi-stage build - `rust:1-slim` builder, `debian:bookworm-slim` runtime. Target: ~15 MB image with `saan_cli`.

`release.yml`: on `v*` tag - build binaries for `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`; upload as GitHub Release assets; publish `saan_ops` to PyPI via `maturin publish`.

---

## 3. Success Criteria

1. `saan prepare manifest.json` produces a correct lineage graph from a real dbt manifest.
2. A plugin `.dll`/`.so` built outside this repo loads via `--plugin`.
3. Parallel ingestion on a 1000-file fixture is >= 4x faster than serial.
4. `docker build .` produces a working image; `docker run saan_cli --help` exits zero.
5. `cargo test --workspace` passes.
