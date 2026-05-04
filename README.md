# Saan

Saan is a data lineage platform: extract how data assets connect, persist the graph, validate it, visualize it. Built on Rust + DuckDB for zero-configuration embedded storage and sub-millisecond graph traversals.

## Components

| Crate | Role |
|-------|------|
| `saan_core` | Library — graph engine, Shaver trait, DuckDB store |
| `saan_cli` | CLI — `saan init / prepare / interlace / apply / inspect / view` |
| `saan_mesh` | WASM visualizer (Phase 3) |
| `saan_ops` | Python SDK via PyO3/maturin (Phase 4) |

## Quick Start

```powershell
cargo build --bin saan_cli

# Create a store
saan_cli init my-project

# Extract lineage from SQL files
saan_cli prepare my-project/ --store my-project/.saan --dialect postgres

# Compute transitive edges
saan_cli interlace --store my-project/.saan

# Promote staging to graph
saan_cli apply --store my-project/.saan

# Validate graph health
saan_cli inspect --store my-project/.saan
```

## CLI Reference

| Command | Status | Description |
|---------|--------|-------------|
| `saan init [path] [--force]` | Done | Create a `.saan` store |
| `saan prepare <input> [--store] [--dialect]` | Done | Walk input files, stage lineage |
| `saan interlace [--store]` | Done | Compute transitive edges in staging |
| `saan apply [--store]` | Done | Promote staging into the final graph |
| `saan inspect [--store]` | Done | Report orphan nodes, cycles, external refs |
| `saan view [--store] [--out]` | Phase 3 | Render graph to HTML+SVG |
| `saan query <sql> [--store] [--format]` | Phase 5 | SQL passthrough on the store |

### Supported SQL dialects (`--dialect`)

`generic` (default) · `ansi` · `postgres` · `mysql` · `mssql` · `bigquery` · `snowflake` · `hive` · `redshift` · `sqlite` · `duckdb` · `clickhouse`

## Python SDK (Phase 4)

```python
import saan_ops

conn = saan_ops.connect("my-project/.saan")
conn.prepare("my-project/", dialect="postgres")
conn.interlace()
conn.apply()

report = conn.inspect()
print(report.total_nodes, report.total_edges)

# Phase 5
df = conn.query("SELECT * FROM nodes").to_pandas()
```

## Building on Windows

The GNU `ld` linker cannot link DuckDB's bundled static library. The repo pins the MSVC toolchain via `rust-toolchain.toml` and adds `rstrtmgr.lib` in `.cargo/config.toml`. No manual steps required — `cargo build` picks this up automatically.

Linux/macOS are unaffected.

## Status

| Phase | Scope | Status |
|-------|-------|--------|
| 1 — Lineage Spine | `init`, `prepare`, `apply`, core graph + store | Done |
| 2 — Validation | `inspect`, `interlace`, SQL dialect config | Done |
| 3 — Visualization | `saan view` HTML+SVG, WASM React mesh | Pending |
| 4 — Python SDK | PyO3 bindings, `maturin` package | Pending |
| 5 — Ad-Hoc Query | `saan query`, `.to_pandas()` | Pending |
| 6 — Ecosystem | dbt Shaver, plugin system, parallel ingestion, Docker | Pending |

## Documentation

- [Roadmap](docs/ROADMAP.md)
- [Technical Specifications](docs/TECHNICAL_SPECIFICATIONS.md)
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md)

## License

MIT
