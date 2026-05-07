"""Tests for saan-ops: package metadata and public API."""

import os
import tempfile
import pytest
import saan_ops


def test_version_is_defined():
    assert hasattr(saan_ops, "__version__")


def test_version_is_semver_shaped():
    parts = saan_ops.__version__.split(".")
    assert len(parts) == 3, "version should be in MAJOR.MINOR.PATCH format"
    assert all(p.isdigit() for p in parts)


def test_connect_is_importable():
    assert callable(saan_ops.connect)


def test_connect_opens_new_store(tmp_path):
    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    assert isinstance(conn, saan_ops.SaanConnection)
    assert os.path.exists(store)


def test_prepare_and_apply_round_trip(tmp_path):
    sql_file = tmp_path / "pipeline.sql"
    sql_file.write_text("CREATE TABLE stg.orders AS SELECT * FROM raw.orders;")

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path))
    conn.apply()

    report = conn.inspect()
    assert report.total_nodes == 2
    assert report.total_edges == 1


def test_interlace_returns_count(tmp_path):
    sql_file = tmp_path / "pipeline.sql"
    sql_file.write_text(
        "CREATE TABLE b AS SELECT * FROM a;\nCREATE TABLE c AS SELECT * FROM b;"
    )

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path))
    count = conn.interlace()
    assert count == 1
    conn.apply()

    report = conn.inspect()
    assert report.total_edges == 3  # a→b, b→c, a→c (interlaced)


def test_inspect_report_fields(tmp_path):
    sql_file = tmp_path / "pipeline.sql"
    sql_file.write_text("CREATE TABLE stg.orders AS SELECT * FROM raw.orders;")

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path))
    conn.apply()

    report = conn.inspect()
    assert isinstance(report.total_nodes, int)
    assert isinstance(report.total_edges, int)
    assert isinstance(report.orphan_nodes, list)
    assert isinstance(report.external_refs, list)
    assert isinstance(report.cycle_detected, bool)
    assert not report.cycle_detected


def test_inspect_detects_orphan(tmp_path):
    sql_file = tmp_path / "pipeline.sql"
    # bare SELECT from standalone_table creates a source node with no edges
    sql_file.write_text("SELECT * FROM standalone_table;")

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path))
    conn.apply()

    report = conn.inspect()
    assert len(report.orphan_nodes) > 0


def test_prepare_with_dialect(tmp_path):
    sql_file = tmp_path / "cast.sql"
    sql_file.write_text("CREATE TABLE t AS SELECT id::text FROM src;")

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path), dialect="postgres")
    conn.apply()

    report = conn.inspect()
    assert report.total_nodes >= 1


def test_inspect_report_repr(tmp_path):
    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    report = conn.inspect()
    assert "InspectReport" in repr(report)


def test_query_returns_query_result(tmp_path):
    sql_file = tmp_path / "pipeline.sql"
    sql_file.write_text("CREATE TABLE stg.orders AS SELECT * FROM raw.orders;")

    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    conn.prepare(str(tmp_path))
    conn.apply()

    result = conn.query("SELECT COUNT(*) AS cnt FROM nodes")
    assert isinstance(result, saan_ops.QueryResult)
    assert result.columns == ["cnt"]
    assert result.rows == [["2"]]


def test_query_empty_result_has_columns(tmp_path):
    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)

    result = conn.query("SELECT id FROM nodes")
    assert result.columns == ["id"]
    assert result.rows == []


def test_query_result_repr(tmp_path):
    store = str(tmp_path / ".saan")
    conn = saan_ops.connect(store)
    result = conn.query("SELECT id FROM nodes")
    assert "QueryResult" in repr(result)
