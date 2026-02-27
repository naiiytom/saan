# Saan

Saan is a high-performance, polymorphic data lineage and visualization platform. It is built as a hybrid system designed for zero-latency metadata operations and "Metadata-as-Code" workflows.

## Core Components

-   **The Weaver (saan_core)**: High-concurrency engine responsible for graph management and automated lineage extraction. Includes The Shaving Layer (Discovery Modules).
-   **The Toolbelt (saan_cli)**: The digital craft station, guiding developers through the artisan's journey of building a data basket.
-   **The Mesh (saan_mesh)**: A standalone, browser-based visualizer that renders data lineage as a high-fidelity "Woven Mesh".
-   **The Hand (Python SDK - saan-ops)**: A developer-friendly wrapper to integrate Saan's weaving logic into existing ETL/ELT pipelines.

## Architecture

Saan leverages the sub-millisecond graph traversals and memory safety of Rust for its core components (The Weaver, The Toolbelt), the analytical power of DuckDB as a zero-configuration embedded database (The Store), and the flexibility of React + WASM so the UI (The Mesh) can run quickly as a standalone HTML file without server dependencies.

## Getting Started

### Prerequisites

-   Rust (latest stable)
-   Python 3.8+
-   Node.js (for UI development)

### Installation

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/naiiytom/saan.git
    cd saan
    ```

2.  **Build Core Components:**
    ```bash
    cargo build --release
    ```

3.  **Install Python SDK:**
    ```bash
    cd sdk/python
    pip install .
    ```

## Usage

### CLI Commands (The Digital Craft Station)

```bash
# Initializes a new Metadata Basket (the local metadata store).
saan init

# (Slicing the Strands) Scans environment to extract raw metadata.
saan prepare

# (Weaving the Pattern) Defines how one metadata asset connects to another.
saan interlace

# Persists the constructed metadata graph.
saan apply

# (Checking the Tension) Scans the structure to identify structural failures.
saan inspect

# (The Woven Basket Visualizer) Compiles metadata into a standalone HTML file and launches a WASM visualizer.
saan view
```

### Python SDK (The Hand)

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

## Documentation

-   [Technical Specifications](docs/TECHNICAL_SPECIFICATIONS.md)
-   [Roadmap](docs/ROADMAP.md)

## License

MIT
