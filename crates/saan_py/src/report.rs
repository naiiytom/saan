use pyo3::prelude::*;
use saan_core::InspectReport;

#[pyclass(unsendable)]
#[derive(Clone)]
pub struct _InspectReport {
    #[pyo3(get)]
    pub total_nodes: usize,
    #[pyo3(get)]
    pub total_edges: usize,
    #[pyo3(get)]
    pub orphan_nodes: Vec<String>,
    #[pyo3(get)]
    pub cycle_detected: bool,
    #[pyo3(get)]
    pub external_refs: Vec<String>,
}

impl From<InspectReport> for _InspectReport {
    fn from(r: InspectReport) -> Self {
        Self {
            total_nodes: r.total_nodes,
            total_edges: r.total_edges,
            orphan_nodes: r.orphan_nodes,
            cycle_detected: r.cycle_detected,
            external_refs: r.external_refs,
        }
    }
}

#[pymethods]
impl _InspectReport {
    fn __repr__(&self) -> String {
        format!(
            "_InspectReport(nodes={}, edges={}, orphans={}, cycle={}, external={})",
            self.total_nodes,
            self.total_edges,
            self.orphan_nodes.len(),
            self.cycle_detected,
            self.external_refs.len(),
        )
    }
}
