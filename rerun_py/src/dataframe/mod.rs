#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod component_columns;
mod index_columns;
mod recording;
mod recording_view;
mod rrd;
mod schema;
mod type_aliases;

pub use self::{
    component_columns::{PyComponentColumnDescriptor, PyComponentColumnSelector},
    index_columns::{PyIndexColumnDescriptor, PyIndexColumnSelector},
    recording::{PyRecording, PyRecordingHandle},
    recording_view::PyRecordingView,
    rrd::{PyRRDArchive, load_archive, load_recording},
    schema::PySchema,
    type_aliases::{AnyColumn, AnyComponentColumn, IndexValuesLike},
};

use pyo3::{
    Bound, PyResult,
    types::{PyModule, PyModuleMethods as _},
    wrap_pyfunction,
};

/// Register the `rerun.dataframe` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchema>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyRecording>()?;
    m.add_class::<PyIndexColumnDescriptor>()?;
    m.add_class::<PyIndexColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;
    m.add_class::<PyRecordingView>()?;

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}
