"""Tests for the saan-ops package metadata and public API surface."""

import importlib
import pytest
import saan_ops


# ---------------------------------------------------------------------------
# Package metadata
# ---------------------------------------------------------------------------

def test_version_is_defined():
    assert hasattr(saan_ops, "__version__")


def test_version_is_semver_shaped():
    parts = saan_ops.__version__.split(".")
    assert len(parts) == 3, "version should be in MAJOR.MINOR.PATCH format"
    assert all(part.isdigit() for part in parts)


# ---------------------------------------------------------------------------
# Public API surface — connect()
# ---------------------------------------------------------------------------

def test_connect_is_importable():
    assert callable(saan_ops.connect)


def test_connect_raises_not_implemented():
    """connect() must raise NotImplementedError until the Rust extension is built."""
    with pytest.raises(NotImplementedError):
        saan_ops.connect("test.saan")


# ---------------------------------------------------------------------------
# SaanConnection API surface
# ---------------------------------------------------------------------------

def test_saan_connection_is_importable():
    assert hasattr(saan_ops, "SaanConnection")


class TestSaanConnectionStubs:
    """Each SaanConnection method must raise NotImplementedError until
    the PyO3 bindings are compiled. These tests document the full API
    surface expected by pipeline integrations.
    """

    def setup_method(self):
        self.conn = saan_ops.SaanConnection("test.saan")

    def test_prepare_raises_not_implemented(self):
        with pytest.raises(NotImplementedError):
            self.conn.prepare("data.csv")

    def test_interlace_raises_not_implemented(self):
        with pytest.raises(NotImplementedError):
            self.conn.interlace()

    def test_apply_raises_not_implemented(self):
        with pytest.raises(NotImplementedError):
            self.conn.apply()

    def test_inspect_raises_not_implemented(self):
        with pytest.raises(NotImplementedError):
            self.conn.inspect()

    def test_query_raises_not_implemented(self):
        with pytest.raises(NotImplementedError):
            self.conn.query("SELECT 1")

    def test_connection_stores_path(self):
        conn = saan_ops.SaanConnection("my_project.saan")
        assert conn.path == "my_project.saan"
