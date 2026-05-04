use pyo3::prelude::*;
use saan_core::{InspectReport, ShaverRegistry, SqlDialect, Store};
use std::path::PathBuf;

use crate::report::_InspectReport;

#[pyclass(unsendable)]
pub struct _SaanConnection {
    store: Store,
    #[allow(dead_code)]
    path: PathBuf,
}

#[pymethods]
impl _SaanConnection {
    #[pyo3(signature = (source, dialect=None))]
    pub fn prepare(&self, source: &str, dialect: Option<&str>) -> PyResult<()> {
        let dialect = parse_dialect(dialect.unwrap_or("generic"))?;
        let registry = ShaverRegistry::new().with_sql_dialect(dialect);

        let source_path = PathBuf::from(source);
        let strands = registry.shave_path(&source_path).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("prepare failed: {e}"))
        })?;
        self.store.write_strands_to_staging(&strands).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("staging write failed: {e}"))
        })?;
        Ok(())
    }

    pub fn apply(&self) -> PyResult<()> {
        self.store
            .apply_staging()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("apply failed: {e}")))?;
        Ok(())
    }

    pub fn interlace(&self) -> PyResult<usize> {
        self.store.interlace_staging().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("interlace failed: {e}"))
        })
    }

    pub fn inspect(&self) -> PyResult<_InspectReport> {
        let r: InspectReport = self.store.inspect().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("inspect failed: {e}"))
        })?;
        Ok(_InspectReport::from(r))
    }

    pub fn query(&self, sql: &str) -> PyResult<crate::query_result::_QueryResult> {
        let r = self
            .store
            .query(sql)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("query failed: {e}")))?;
        Ok(crate::query_result::_QueryResult::from(r))
    }
}

#[pyfunction]
pub fn _connect(path: &str) -> PyResult<_SaanConnection> {
    let p = PathBuf::from(path);
    let store = Store::open(&p).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("failed to open store at {path}: {e}"))
    })?;
    store.init_schema().map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("schema init failed: {e}"))
    })?;
    Ok(_SaanConnection { store, path: p })
}

fn parse_dialect(s: &str) -> PyResult<SqlDialect> {
    match s {
        "generic" => Ok(SqlDialect::Generic),
        "ansi" => Ok(SqlDialect::Ansi),
        "postgres" => Ok(SqlDialect::Postgres),
        "mysql" => Ok(SqlDialect::MySql),
        "mssql" => Ok(SqlDialect::MsSql),
        "bigquery" => Ok(SqlDialect::BigQuery),
        "snowflake" => Ok(SqlDialect::Snowflake),
        "hive" => Ok(SqlDialect::Hive),
        "redshift" => Ok(SqlDialect::Redshift),
        "sqlite" => Ok(SqlDialect::SQLite),
        "duckdb" => Ok(SqlDialect::DuckDb),
        "clickhouse" => Ok(SqlDialect::ClickHouse),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown dialect: {other}"
        ))),
    }
}
