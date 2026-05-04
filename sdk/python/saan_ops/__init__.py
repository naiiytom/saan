"""saan-ops: Python SDK for Saan — data lineage platform."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pandas
    import polars
    import pyarrow

__version__ = "0.1.0"

from saan_ops._core import _SaanConnection, _InspectReport, _QueryResult, _connect


class InspectReport:
    """Structural health report for a Saan graph."""

    def __init__(self, inner: _InspectReport) -> None:
        self._inner = inner

    @property
    def total_nodes(self) -> int:
        return self._inner.total_nodes

    @property
    def total_edges(self) -> int:
        return self._inner.total_edges

    @property
    def orphan_nodes(self) -> list[str]:
        return self._inner.orphan_nodes

    @property
    def cycle_detected(self) -> bool:
        return self._inner.cycle_detected

    @property
    def external_refs(self) -> list[str]:
        return self._inner.external_refs

    def __repr__(self) -> str:
        return (
            f"InspectReport(nodes={self.total_nodes}, edges={self.total_edges}, "
            f"orphans={len(self.orphan_nodes)}, cycle={self.cycle_detected}, "
            f"external={len(self.external_refs)})"
        )


class QueryResult:
    """Result of a SQL query against the store."""

    def __init__(self, inner: _QueryResult) -> None:
        self._inner = inner

    @property
    def columns(self) -> list[str]:
        return self._inner.columns

    @property
    def rows(self) -> list[list[str]]:
        return self._inner.rows

    def to_pandas(self) -> "pandas.DataFrame":
        import pandas as pd
        return pd.DataFrame(self.rows, columns=self.columns)

    def to_polars(self) -> "polars.DataFrame":
        import polars as pl
        data = {col: [row[i] for row in self.rows] for i, col in enumerate(self.columns)}
        return pl.DataFrame(data)

    def to_arrow(self) -> "pyarrow.Table":
        import pyarrow as pa
        data = {col: [row[i] for row in self.rows] for i, col in enumerate(self.columns)}
        return pa.table(data)

    def __repr__(self) -> str:
        return f"QueryResult(columns={self.columns!r}, rows={len(self.rows)})"


class SaanConnection:
    """Handle to an open Saan metadata store."""

    def __init__(self, inner: _SaanConnection) -> None:
        self._inner = inner

    def prepare(self, source: str, *, dialect: str = "generic") -> None:
        """Walk *source* (file or directory) and stage lineage into the store."""
        self._inner.prepare(source, dialect)

    def apply(self) -> None:
        """Promote staged lineage into the final graph tables."""
        self._inner.apply()

    def interlace(self) -> int:
        """Compute transitive edges in staging. Returns number of edges added."""
        return self._inner.interlace()

    def inspect(self) -> InspectReport:
        """Return a structural health report for the current graph."""
        return InspectReport(self._inner.inspect())

    def query(self, sql: str) -> QueryResult:
        """Execute *sql* against the store and return a :class:`QueryResult`."""
        return QueryResult(self._inner.query(sql))


def connect(path: str) -> SaanConnection:
    """Open (or create) a Saan store at *path* and return a :class:`SaanConnection`."""
    return SaanConnection(_connect(path))
