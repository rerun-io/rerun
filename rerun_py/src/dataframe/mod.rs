#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod component_columns;
mod index_columns;
mod recording;
mod recording_view;
mod rrd;
mod schema;
mod type_aliases;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult, wrap_pyfunction};

pub use self::component_columns::{PyComponentColumnDescriptor, PyComponentColumnSelector};
pub use self::index_columns::{PyIndexColumnDescriptor, PyIndexColumnSelector};
pub use self::recording::{PyRecording, PyRecordingHandle};
pub use self::recording_view::PyRecordingView;
pub use self::rrd::{PyRRDArchive, load_archive, load_recording};
pub use self::schema::PySchemaInternal;
pub use self::type_aliases::{AnyColumn, AnyComponentColumn, IndexValuesLike, PyIndexValuesLike};

/// Register the `rerun.dataframe` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchemaInternal>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyRecording>()?;
    m.add_class::<PyIndexColumnDescriptor>()?;
    m.add_class::<PyIndexColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;
    m.add_class::<PyRecordingView>()?;
    m.add_class::<PyIndexValuesLike>()?;

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}
