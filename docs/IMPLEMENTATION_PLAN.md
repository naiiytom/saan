# Implementation Plan — Phase 1 Lineage MVP

**Status:** Proposed, awaiting review
**Scope:** The Phase 1 milestone in `ROADMAP.md` — the thinnest end-to-end lineage spine: `saan init` + `saan prepare` (with one `SqlShaver`) + `saan apply`, persisting a real graph to a real `.saan` file.

This plan is the work breakdown for Phase 1 only. Phase 2+ get their own plans when their phase begins.

---

## 1. Goal

A user can do this from a fresh clone:

```bash
cargo build --release
mkdir my-pipeline && cd my-pipeline
saan init
echo "CREATE TABLE marts.orders AS SELECT * FROM raw.orders" > model.sql
saan prepare ./model.sql
saan apply
duckdb project.saan -c "SELECT * FROM nodes; SELECT * FROM edges;"
# Returns: 2 nodes (raw.orders, marts.orders), 1 edge (raw.orders → marts.orders)
```

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

New module layout:

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

Dependencies added to `crates/saan_core/Cargo.toml`:

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

New layout:

```
saan_cli/src/
    main.rs             — clap entry point + dispatch
    commands/
        mod.rs
        init.rs
        prepare.rs
        apply.rs
```

Dependencies added to `crates/saan_cli/Cargo.toml`:

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
    pub fn new(source_path: impl Into<PathBuf>) -> Self;
    pub fn add_node(&mut self, node: Node) -> &mut Self;
    pub fn add_edge(&mut self, edge: Edge) -> &mut Self;
}
```

### 4.3 `ShaverError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum ShaverError {
    #[error("io error reading {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{0}")]
    Other(String),
}
```

### 4.4 `ShaverRegistry`

```rust
pub struct ShaverRegistry {
    by_extension: HashMap<String, Arc<dyn Shaver>>,
}

impl ShaverRegistry {
    pub fn new() -> Self;
    pub fn with_builtins() -> Self;  // SqlShaver pre-registered
    pub fn register(&mut self, shaver: Arc<dyn Shaver>) -> &mut Self;
    pub fn for_extension(&self, ext: &str) -> Option<&Arc<dyn Shaver>>;
}
```

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
- Default path: `./project.saan`.
- Refuses if file exists; `--force` overwrites.
- Opens DuckDB at path, runs schema DDL (Section 7), closes.

### 6.2 `saan prepare <input> [--store path]`
- Default store: `./project.saan` (errors if missing).
- Walks `<input>` recursively (also accepts a single file).
- Per file: look up Shaver by extension via `ShaverRegistry::with_builtins()`. Unknown extensions → debug log + skip.
- Each `shave()` returns a Strand. Strands accumulate in memory.
- End of run: write all Strands into `staging_nodes` / `staging_edges` in one transaction. Final `nodes` / `edges` not touched.
- Print summary: files scanned, files shaved, strands produced, errors.

### 6.3 `saan apply [--store path]`
- Reads staging tables.
- One transaction: UPSERT staging into final `nodes` / `edges`, then truncate staging.
- Timestamp behavior: on insert, `first_seen_at = last_seen_at = now()`. On update (id collision), `first_seen_at` is preserved and `last_seen_at = now()`. The Store sets these — the Strand does not carry timestamps.
- Print summary: N nodes added/updated, M edges added/updated.

### 6.4 Why staging-then-apply (not direct write)

The spec models lineage construction as three steps (prepare → interlace → apply). `interlace` is a no-op in MVP, but staging preserves the pipeline shape without surgery later. It also gives `apply` a real, distinguishable job — a user can `prepare` against five directories and then `apply` once when satisfied.

## 7. `.saan` File Schema (DuckDB)

```sql
CREATE TABLE saan_meta (
  key VARCHAR PRIMARY KEY,
  value VARCHAR
);
INSERT INTO saan_meta VALUES ('schema_version', '1');

CREATE TABLE nodes (
  id            VARCHAR PRIMARY KEY,
  label         VARCHAR NOT NULL,
  source_type   VARCHAR NOT NULL,
  first_seen_at TIMESTAMP NOT NULL,
  last_seen_at  TIMESTAMP NOT NULL
);

CREATE TABLE edges (
  from_id VARCHAR NOT NULL,
  to_id   VARCHAR NOT NULL,
  PRIMARY KEY (from_id, to_id)
);

CREATE TABLE staging_nodes (
  id          VARCHAR NOT NULL,
  label       VARCHAR NOT NULL,
  source_type VARCHAR NOT NULL,
  source_path VARCHAR
);

CREATE TABLE staging_edges (
  from_id     VARCHAR NOT NULL,
  to_id       VARCHAR NOT NULL,
  source_path VARCHAR
);
```

**Two deliberate non-decisions:**

- **No FK from `edges` to `nodes`.** DuckDB's FK support is limited; lineage routinely points at nodes from systems we have not (yet) ingested. `inspect` (Phase 2) reports orphans, but they are not a constraint failure.
- **`source_path` is metadata, not identity.** Two SQL files referencing `raw.orders` produce one node, not two. The Shaver normalizes; `source_path` just lets us trace which file contributed which staging row.

## 8. Testing Strategy

The codebase already has good unit-test discipline (Node/Edge/Graph, CLI parser, RenderConfig all have tests). Build on that pattern; do not introduce a new test framework.

### 8.1 Unit tests (colocated with each module)

- `graph` — preserve current cases against the new petgraph-backed `Graph`; verify the wrapper exposes cycle detection.
- `strand` — construction, `add_node` / `add_edge` chaining.
- `shaver::registry` — `with_builtins()` resolves `.sql` to `SqlShaver`; `register()` overrides; `for_extension("xyz")` returns `None`.
- `shavers::sql` — table-driven cases covering each row of Section 5.1 plus: CTE exclusion, nested CTEs, subqueries in FROM, schema/database qualification, quoted-identifier case, parse error, non-UTF-8 input. Inline SQL strings.
- `store` — open/create, init schema (verify all 5 tables), save Strand to staging, apply moves staging → final, **re-apply is a no-op** (idempotence).

### 8.2 Integration tests (`crates/saan_core/tests/`)

Full library pipeline against fixture files in `tests/fixtures/sql/`. 3-5 fixtures: a minimal one, one with CTEs, one with multiple statements, one with a parse error.

Open a tempfile-backed store, run registry → SqlShaver → staging → apply, query `nodes` / `edges`, assert exact contents.

### 8.3 CLI integration tests (`crates/saan_cli/tests/cli_integration.rs`)

`assert_cmd` is already wired in. Add:

- `saan init` creates the file; second `init` without `--force` fails; `--force` overwrites.
- `saan init && saan prepare <fixture> && saan apply` against a known fixture produces a `.saan` whose tables match expected contents (open with the `duckdb` crate in the test).
- `interlace` / `inspect` / `view` print "not implemented in Phase 1" and exit non-zero.

## 9. Success Criteria

1. `cargo test --workspace` passes.
2. The fresh-clone flow in Section 1 produces the expected rows.
3. Re-running `prepare` + `apply` on the same input adds zero new rows (end-to-end idempotence).
4. Phase-2 commands (`interlace`, `inspect`, `view`) exit cleanly with a "not implemented" message — no panics, no removed enum variants.
5. `docs/ROADMAP.md` and `docs/TECHNICAL_SPECIFICATIONS.md` are aligned with this plan (this is already done as part of the same change set).

## 10. Suggested Implementation Order

Each step ends in a green `cargo test --workspace`.

1. **Wire deps.** Add petgraph, sqlparser, duckdb, thiserror, walkdir to `saan_core`; add clap, anyhow to `saan_cli`. `cargo check`.
2. **Replace Graph.** Swap custom `Graph` for the petgraph wrapper; preserve the existing test suite's expectations.
3. **`Strand`, `ShaverError`, `Shaver` trait, `ShaverRegistry`.** All in `saan_core::shaver`. Tests for the registry.
4. **`SqlShaver` — minimal cases first.** Bare SELECT, CREATE TABLE AS, JOIN. Then INSERT, then CTEs, then subqueries, then qualified names.
5. **`Store` — schema only.** `open`, `init_schema`. Test: opening an empty file produces all 5 tables.
6. **`Store` — staging writes.** `write_strands_to_staging`. Test idempotent rewrites.
7. **`Store` — apply.** `apply_staging` UPSERT logic. Test idempotent re-apply.
8. **`Store` — load_graph.** Pull final tables back into a `Graph`. Round-trip test.
9. **CLI — clap skeleton.** Move from hand-rolled args to `clap`. Phase-2 commands print "not implemented" and exit 1.
10. **CLI — `init`.** Wire into Store. Test with `assert_cmd`.
11. **CLI — `prepare`.** Wire walkdir + ShaverRegistry + Store staging writes.
12. **CLI — `apply`.** Wire Store::apply_staging.
13. **End-to-end CLI integration test.** The fresh-clone flow from Section 1.
