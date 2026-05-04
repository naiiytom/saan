use pyo3::prelude::*;
use saan_core::QueryResult;

#[pyclass(unsendable)]
pub struct _QueryResult {
    #[pyo3(get)]
    pub columns: Vec<String>,
    #[pyo3(get)]
    pub rows: Vec<Vec<String>>,
}

impl From<QueryResult> for _QueryResult {
    fn from(r: QueryResult) -> Self {
        Self {
            columns: r.columns,
            rows: r.rows,
        }
    }
}

#[pymethods]
impl _QueryResult {
    fn __repr__(&self) -> String {
        format!(
            "_QueryResult(columns={:?}, rows={})",
            self.columns,
            self.rows.len()
        )
    }
}
