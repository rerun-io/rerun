mod types;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult, wrap_pyfunction};

pub use self::types::{PyChunkInternal, PyChunkIterator};

/// Register the `rerun.chunk` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyChunkInternal>()?;
    m.add_class::<PyChunkIterator>()?;

    m.add_function(wrap_pyfunction!(types::recording_from_chunks, m)?)?;

    Ok(())
}
