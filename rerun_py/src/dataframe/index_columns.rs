use pyo3::{pyclass, pymethods};
use re_sorbet::{IndexColumnDescriptor, TimeColumnSelector};

/// The descriptor of an index column.
///
/// Index columns contain the index values for when the data was updated. They
/// generally correspond to Rerun timelines.
///
/// Column descriptors are used to describe the columns in a
/// [`Schema`][rerun.dataframe.Schema]. They are read-only. To select an index
/// column, use [`IndexColumnSelector`][rerun.dataframe.IndexColumnSelector].
#[pyclass(frozen, name = "IndexColumnDescriptor")]
#[derive(Clone)]
pub struct PyIndexColumnDescriptor(pub IndexColumnDescriptor);

#[pymethods]
impl PyIndexColumnDescriptor {
    fn __repr__(&self) -> String {
        format!("Index(timeline:{})", self.0.column_name())
    }

    /// The name of the index.
    ///
    /// This property is read-only.
    #[getter]
    fn name(&self) -> &str {
        self.0.column_name()
    }

    /// Part of generic ColumnDescriptor interface: always False for Index.
    #[expect(clippy::unused_self)]
    #[getter]
    fn is_static(&self) -> bool {
        false
    }
}

impl From<IndexColumnDescriptor> for PyIndexColumnDescriptor {
    fn from(desc: IndexColumnDescriptor) -> Self {
        Self(desc)
    }
}

/// A selector for an index column.
///
/// Index columns contain the index values for when the data was updated. They
/// generally correspond to Rerun timelines.
///
/// Parameters
/// ----------
/// index : str
///     The name of the index to select. Usually the name of a timeline.
#[pyclass(frozen, name = "IndexColumnSelector")]
#[derive(Clone)]
pub struct PyIndexColumnSelector(pub TimeColumnSelector);

#[pymethods]
impl PyIndexColumnSelector {
    /// Create a new `IndexColumnSelector`.
    // Note: the `Parameters` section goes into the class docstring.
    #[new]
    #[pyo3(text_signature = "(self, index)")]
    fn new(index: &str) -> Self {
        Self(TimeColumnSelector::from(index))
    }

    fn __repr__(&self) -> String {
        format!("Index(timeline:{})", self.0.timeline)
    }

    /// The name of the index.
    ///
    /// This property is read-only.
    #[getter]
    fn name(&self) -> &str {
        &self.0.timeline
    }
}

impl From<PyIndexColumnSelector> for TimeColumnSelector {
    fn from(selector: PyIndexColumnSelector) -> Self {
        selector.0
    }
}
