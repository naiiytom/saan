# Technical Specifications

## 1. Overview
Saan is a high-performance data processing and visualization platform. It is built as a hybrid system designed for zero-latency metadata operations and "Metadata-as-Code" workflows. It leverages Rust for core performance, DuckDB for in-process SQL analytics, WebAssembly (WASM) for browser-based visualization, and Python for data science integration.

## 2. Architecture

### 2.1 Core Components

*   **The Weaver (Rust Core):**
    *   **Responsibility:** High-concurrency engine responsible for graph management and automated lineage extraction.
    *   **Key Features:**
        *   Implements a "Polymorphic Ingestion Engine" to handle diverse data sources.
        *   Graph Logic using `petgraph` for DAG management, cycle detection, and reachability analysis.
        *   Logic Parser using `sqlparser-rs` for vendor-agnostic SQL parsing to automate lineage extraction and interlacing.

*   **The Shaving Layer (Discovery Modules):**
    *   **Responsibility:** Uses specialized "Shavers" (implementing a common Rust trait) to parse different formats.
    *   **Key Features:**
        *   SQL Shavers, Manifest Shavers (for dbt), File Shavers (for JSON/YAML/TOML).
        *   Blob Shavers (for Parquet/CSV), and BI Shavers (for Tableau/PowerBI).

*   **The Store (DuckDB):**
    *   **Responsibility:** Persisting your global graph locally.
    *   **Key Features:**
        *   Zero-configuration, embedded database that is extremely fast for analytical queries.
        *   Stores the graph in an embedded `.saan` file.

*   **The Mesh (WASM UI):**
    *   **Responsibility:** Interactive data visualization and exploration.
    *   **Key Features:**
        *   Standalone, browser-based visualizer.
        *   Renders data lineage as a high-fidelity "Woven Mesh" rather than standard node-link diagrams.
    *   **Tech Stack:** React + WASM (WebAssembly). Runs quickly as a standalone HTML file without server dependencies.

*   **The Hand (Python SDK - saan-ops):**
    *   **Responsibility:** Python bindings for integrating Saan logic.
    *   **Key Features:**
        *   Developer-friendly wrapper to integrate Saan's weaving logic directly into existing ETL/ELT pipelines (like Airflow, Dagster, or Prefect).

### 2.2 Data Flow

1.  **Ingestion (Prepare):** Data is read by The Shaving Layer from various sources. Scans environment to extract raw metadata.
2.  **Processing (Interlace):** Defines how metadata assets connect to another. Builds graph edges manually or automatically.
3.  **Persistence (Apply):** The global graph is persisted locally in the embedded DuckDB database (`.saan` file).
4.  **Verification (Inspect):** Scans the structure to identify structural failures, broken lineage, orphaned tables, schema drift, or data quality gaps.
5.  **Visualization (View):** Compiles metadata into a standalone HTML file and launches the WASM-powered visualizer (The Woven Basket Visualizer) to display data flow and dynamic tension.

## 3. Technology Stack

*   **Language:** Rust (chosen for sub-millisecond graph traversals and memory safety)
*   **Concurrency Model:** Tokio (asynchronous metadata extraction from remote sources like Cloud DBs and APIs)
*   **Database Engine:** DuckDB
*   **WASM Framework:** React + WASM
*   **Python Interop:** PyO3 / Maturin
*   **Graph Logic:** petgraph
*   **Logic Parser:** sqlparser-rs

## 4. Interfaces

### 4.1 CLI Commands (The Digital Craft Station)

*   `saan init`: Initializes a new Metadata Basket (the local metadata store).
*   `saan prepare`: (Slicing the Strands) Scans your environment (SQL files, dbt manifests, Snowflake schemas, local Parquet files) to extract raw metadata and standardize it into clean JSON metadata strips (Strands).
*   `saan interlace`: (Weaving the Pattern) Defines how one metadata asset connects to another. Builds the edges of your graph, either manually or automatically (by parsing SQL logic or dbt macros).
*   `saan apply`: Applies and persists the constructed metadata graph into the local store.
*   `saan inspect`: (Checking the Tension) Scans the structure to identify structural failures, such as broken lineage links, orphaned tables, schema drift, or data quality gaps before they cause reporting failures.
*   `saan view`: (The Woven Basket Visualizer) Compiles your metadata into a standalone HTML file and launches a WASM-powered visualizer.

### 4.2 Python API

```python
import saan_ops

# Initialize
db = saan_ops.connect("my_project.saan")

# Ingest and Prepare
db.prepare("data.csv")

# Interlace Connections
db.interlace()

# Query
df = db.query("SELECT * FROM data").to_pandas()
```

## 5. Security & Performance

*   **Memory Safety:** Guaranteed by Rust.
*   **Concurrency:** Async runtime (Tokio) for I/O bound tasks.
*   **Sandboxing:** WASM provides a secure execution environment for the UI.
