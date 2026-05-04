mod connection;
mod query_result;
mod report;

use pyo3::prelude::*;

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<connection::_SaanConnection>()?;
    m.add_class::<report::_InspectReport>()?;
    m.add_class::<query_result::_QueryResult>()?;
    m.add_function(wrap_pyfunction!(connection::_connect, m)?)?;
    Ok(())
}
