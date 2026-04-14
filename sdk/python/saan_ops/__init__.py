"""
saan-ops: Python SDK for Saan — The Hand.

Provides developer-friendly bindings for integrating Saan's data lineage
weaving logic into ETL/ELT pipelines (Airflow, Dagster, Prefect, etc.).

Planned API surface:

    import saan_ops
    db = saan_ops.connect("my_project.saan")
    db.prepare("data.csv")
    db.interlace()
    df = db.query("SELECT * FROM data").to_pandas()
"""

__version__ = "0.1.0"


def connect(path: str):
    """Open (or create) a Saan metadata store at *path*.

    Returns a :class:`SaanConnection` instance.

    .. note::
        Full implementation requires PyO3/Maturin bindings to the Rust core.
        This stub documents the intended interface.
    """
    raise NotImplementedError(
        "connect() requires the compiled Rust extension. "
        "Run `maturin develop` to build it."
    )


class SaanConnection:
    """Handle to an open Saan metadata store.

    All methods are stubs pending PyO3 bindings.
    """

    def __init__(self, path: str) -> None:
        self.path = path

    def prepare(self, source: str) -> None:
        """Ingest raw metadata from *source* (file path, glob, or connection string)."""
        raise NotImplementedError

    def interlace(self) -> None:
        """Build lineage edges from ingested metadata."""
        raise NotImplementedError

    def apply(self) -> None:
        """Persist the constructed graph to the local store."""
        raise NotImplementedError

    def inspect(self) -> list:
        """Return a list of structural issues found in the graph."""
        raise NotImplementedError

    def query(self, sql: str):
        """Execute *sql* against the store and return a query result."""
        raise NotImplementedError
