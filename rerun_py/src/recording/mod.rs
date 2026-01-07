mod rrd;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult, wrap_pyfunction};

pub use self::rrd::{PyRRDArchive, PyRecording, load_archive, load_recording};

/// Register the `rerun.recording` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyRecording>()?;

    m.add_function(wrap_pyfunction!(load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(load_recording, m)?)?;

    Ok(())
}
