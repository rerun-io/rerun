mod types;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult};

pub use self::types::PyChunkInternal;

/// Register the `rerun.chunk` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyChunkInternal>()?;

    Ok(())
}
