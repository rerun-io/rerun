#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(deprecated)] // False positive due to macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr as _,
    sync::Arc,
};

use arrow::{
    array::{make_array, ArrayData, Int64Array, RecordBatchIterator, RecordBatchReader},
    pyarrow::PyArrowType,
};
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use numpy::PyArrayMethods as _;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyCapsule, PyDict, PyTuple},
    IntoPyObjectExt as _,
};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ColumnDescriptor, ComponentColumnDescriptor,
    IndexColumnDescriptor, QueryExpression, SparseFillStrategy, ViewContentsSelector,
};
use re_dataframe::{QueryEngine, StorageEngine};
use re_log_types::{EntityPathFilter, ResolvedTimeRange};
use re_sdk::{ComponentName, EntityPath, StoreId, StoreKind};
use re_sorbet::{
    ColumnSelector, ComponentColumnSelector, SorbetColumnDescriptors, TimeColumnSelector,
};

use crate::catalog::to_py_err;
use crate::{catalog::PyCatalogClient, utils::get_tokio_runtime};

/// Register the `rerun.dataframe` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchema>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyRecording>()?;
    m.add_class::<PyIndexColumnDescriptor>()?;
    m.add_class::<PyIndexColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;
    m.add_class::<PyRecordingView>()?;
    m.add_class::<PyDataFusionTable>()?;

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}

fn py_rerun_warn(msg: &std::ffi::CStr) -> PyResult<()> {
    Python::with_gil(|py| {
        let warning_type = PyModule::import(py, "rerun")?
            .getattr("error_utils")?
            .getattr("RerunWarning")?;
        PyErr::warn(py, &warning_type, msg, 0)?;
        Ok(())
    })
}

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
struct PyIndexColumnDescriptor(IndexColumnDescriptor);

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
    #[allow(clippy::unused_self)]
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
pub struct PyIndexColumnSelector(TimeColumnSelector);

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

/// The descriptor of a component column.
///
/// Component columns contain the data for a specific component of an entity.
///
/// Column descriptors are used to describe the columns in a
/// [`Schema`][rerun.dataframe.Schema]. They are read-only. To select a component
/// column, use [`ComponentColumnSelector`][rerun.dataframe.ComponentColumnSelector].
#[pyclass(frozen, name = "ComponentColumnDescriptor")]
#[derive(Clone)]
pub struct PyComponentColumnDescriptor(pub ComponentColumnDescriptor);

impl From<ComponentColumnDescriptor> for PyComponentColumnDescriptor {
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self(desc)
    }
}

#[pymethods]
impl PyComponentColumnDescriptor {
    fn __repr__(&self) -> String {
        format!(
            "Component({}:{})",
            self.0.entity_path,
            self.0.component_name.short_name()
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    /// The entity path.
    ///
    /// This property is read-only.
    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    /// The component name.
    ///
    /// This property is read-only.
    #[getter]
    fn component_name(&self) -> &str {
        self.0.component_name.full_name()
    }

    /// Whether the column is static.
    ///
    /// This property is read-only.
    #[getter]
    fn is_static(&self) -> bool {
        self.0.is_static
    }
}

impl From<PyComponentColumnDescriptor> for ComponentColumnDescriptor {
    fn from(desc: PyComponentColumnDescriptor) -> Self {
        desc.0
    }
}

/// A selector for a component column.
///
/// Component columns contain the data for a specific component of an entity.
///
/// Parameters
/// ----------
/// entity_path : str
///     The entity path to select.
/// component : ComponentLike
///     The component to select
#[pyclass(frozen, name = "ComponentColumnSelector")]
#[derive(Clone)]
pub struct PyComponentColumnSelector(pub ComponentColumnSelector);

#[pymethods]
impl PyComponentColumnSelector {
    /// Create a new `ComponentColumnSelector`.
    // Note: the `Parameters` section goes into the class docstring.
    #[new]
    #[pyo3(text_signature = "(self, entity_path: str, component: ComponentLike)")]
    fn new(entity_path: &str, component_name: ComponentLike) -> Self {
        Self(ComponentColumnSelector {
            entity_path: entity_path.into(),
            component_name: component_name.0,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Component({}:{})",
            self.0.entity_path, self.0.component_name
        )
    }

    /// The entity path.
    ///
    /// This property is read-only.
    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    /// The component name.
    ///
    /// This property is read-only.
    #[getter]
    fn component_name(&self) -> &str {
        &self.0.component_name
    }
}

impl From<PyComponentColumnSelector> for ComponentColumnSelector {
    fn from(selector: PyComponentColumnSelector) -> Self {
        selector.0
    }
}

/// A type alias for any component-column-like object.
#[derive(FromPyObject)]
enum AnyColumn {
    #[pyo3(transparent, annotation = "name")]
    Name(String),
    #[pyo3(transparent, annotation = "index_descriptor")]
    IndexDescriptor(PyIndexColumnDescriptor),
    #[pyo3(transparent, annotation = "index_selector")]
    IndexSelector(PyIndexColumnSelector),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyColumn {
    fn into_selector(self) -> PyResult<ColumnSelector> {
        match self {
            Self::Name(name) => {
                if !name.contains(':') && !name.contains('/') {
                    Ok(ColumnSelector::Time(TimeColumnSelector::from(name)))
                } else {
                    let component_path =
                        re_log_types::ComponentPath::from_str(&name).map_err(|err| {
                            PyValueError::new_err(format!("Invalid component path {name:?}: {err}"))
                        })?;

                    Ok(ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: component_path.entity_path,
                        component_name: component_path.component_name.to_string(),
                    }))
                }
            }
            Self::IndexDescriptor(desc) => Ok(ColumnDescriptor::Time(desc.0).into()),
            Self::IndexSelector(selector) => Ok(selector.0.into()),
            Self::ComponentDescriptor(desc) => Ok(ColumnDescriptor::Component(desc.0).into()),
            Self::ComponentSelector(selector) => Ok(selector.0.into()),
        }
    }
}

/// A type alias for any component-column-like object.
//TODO(#9853): rename to `ComponentColumnLike`
#[derive(FromPyObject)]
pub enum AnyComponentColumn {
    #[pyo3(transparent, annotation = "name")]
    Name(String),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyComponentColumn {
    #[allow(dead_code)]
    pub fn into_selector(self) -> PyResult<ComponentColumnSelector> {
        match self {
            Self::Name(name) => {
                let component_path =
                    re_log_types::ComponentPath::from_str(&name).map_err(|err| {
                        PyValueError::new_err(format!("Invalid component path '{name}': {err}"))
                    })?;

                Ok(ComponentColumnSelector {
                    entity_path: component_path.entity_path,
                    component_name: component_path.component_name.to_string(),
                })
            }
            Self::ComponentDescriptor(desc) => Ok(desc.0.into()),
            Self::ComponentSelector(selector) => Ok(selector.0),
        }
    }
}

/// A type alias for index values.
///
/// This can be any numpy-compatible array of integers, or a [`pa.Int64Array`][]
#[derive(FromPyObject)]
pub(crate) enum IndexValuesLike<'py> {
    PyArrow(PyArrowType<ArrayData>),
    NumPy(numpy::PyArrayLike1<'py, i64>),

    // Catch all to support ChunkedArray and other types
    #[pyo3(transparent)]
    CatchAll(Bound<'py, PyAny>),
}

impl IndexValuesLike<'_> {
    pub(crate) fn to_index_values(&self) -> PyResult<BTreeSet<re_chunk_store::TimeInt>> {
        match self {
            Self::PyArrow(array) => {
                let array = make_array(array.0.clone());

                let int_array = array.downcast_array_ref::<Int64Array>().ok_or_else(|| {
                    PyTypeError::new_err("pyarrow.Array for IndexValuesLike must be of type int64.")
                })?;

                let values: BTreeSet<re_chunk_store::TimeInt> = int_array
                    .iter()
                    .map(|v| {
                        v.map_or_else(
                            || re_chunk_store::TimeInt::STATIC,
                            // The use of temporal here should be fine even if the data is
                            // not actually temporal. The important thing is we are converting
                            // from an i64 input
                            re_chunk_store::TimeInt::new_temporal,
                        )
                    })
                    .collect();

                if values.len() != int_array.len() {
                    return Err(PyValueError::new_err("Index values must be unique."));
                }

                Ok(values)
            }
            Self::NumPy(array) => {
                let values: BTreeSet<re_chunk_store::TimeInt> = array
                    .readonly()
                    .as_array()
                    .iter()
                    // The use of temporal here should be fine even if the data is
                    // not actually temporal. The important thing is we are converting
                    // from an i64 input
                    .map(|v| re_chunk_store::TimeInt::new_temporal(*v))
                    .collect();

                if values.len() != array.len()? {
                    return Err(PyValueError::new_err("Index values must be unique."));
                }

                Ok(values)
            }
            Self::CatchAll(any) => {
                // If any has the `.chunks` attribute, we can try to try each chunk as pyarrow array
                if let Ok(chunks) = any.getattr("chunks") {
                    let mut values = BTreeSet::new();
                    for chunk in chunks.try_iter()? {
                        let chunk = chunk?.extract::<PyArrowType<ArrayData>>()?;
                        let array = make_array(chunk.0.clone());

                        let int_array =
                            array.downcast_array_ref::<Int64Array>().ok_or_else(|| {
                                PyTypeError::new_err(
                                    "pyarrow.Array for IndexValuesLike must be of type int64.",
                                )
                            })?;

                        values.extend(
                            int_array
                                .iter()
                                .map(|v| {
                                    v.map_or_else(
                                        || re_chunk_store::TimeInt::STATIC,
                                        // The use of temporal here should be fine even if the data is
                                        // not actually temporal. The important thing is we are converting
                                        // from an i64 input
                                        re_chunk_store::TimeInt::new_temporal,
                                    )
                                })
                                .collect::<BTreeSet<_>>(),
                        );
                    }

                    if values.len() != any.len()? {
                        return Err(PyValueError::new_err("Index values must be unique."));
                    }

                    Ok(values)
                } else {
                    Err(PyTypeError::new_err(
                        "IndexValuesLike must be a pyarrow.Array, pyarrow.ChunkedArray, or numpy.ndarray",
                    ))
                }
            }
        }
    }
}

pub struct ComponentLike(pub String);

impl FromPyObject<'_> for ComponentLike {
    fn extract_bound(component: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(component_str) = component.extract::<String>() {
            Ok(Self(component_str))
        } else if let Ok(component_str) = component
            .getattr("_BATCH_TYPE")
            .and_then(|batch_type| batch_type.getattr("_COMPONENT_DESCRIPTOR"))
            .and_then(|descr| descr.getattr("component_name")?.extract::<String>())
        {
            Ok(Self(component_str))
        } else {
            return Err(PyTypeError::new_err(
                "ComponentLike input must be a string or Component class.",
            ));
        }
    }
}

#[pyclass]
pub struct SchemaIterator {
    iter: std::vec::IntoIter<PyObject>,
}

#[pymethods]
impl SchemaIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        slf.iter.next()
    }
}

#[pyclass(frozen, name = "Schema")]
#[derive(Clone)]
//TODO(#9457): improve this object and use it for `Dataset.schema()`.
pub struct PySchema {
    pub schema: SorbetColumnDescriptors,
}

/// The schema representing a set of available columns.
///
/// Can be returned by [`Recording.schema()`][rerun.dataframe.Recording.schema] or
/// [`RecordingView.schema()`][rerun.dataframe.RecordingView.schema].
#[pymethods]
impl PySchema {
    /// Iterate over all the column descriptors in the schema, ignoring `RowId`.
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<SchemaIterator>> {
        let py = slf.py();
        let iter = SchemaIterator {
            iter: slf
                .schema
                .indices_and_components()
                .into_iter()
                .map(|col| match col {
                    ColumnDescriptor::Time(col) => PyIndexColumnDescriptor(col).into_py_any(py),
                    ColumnDescriptor::Component(col) => {
                        PyComponentColumnDescriptor(col).into_py_any(py)
                    }
                })
                .collect::<PyResult<Vec<_>>>()?
                .into_iter(),
        };
        Py::new(slf.py(), iter)
    }

    /// Return a list of all the index columns in the schema.
    fn index_columns(&self) -> Vec<PyIndexColumnDescriptor> {
        self.schema
            .indices
            .iter()
            .map(|c| c.clone().into())
            .collect()
    }

    /// Return a list of all the component columns in the schema.
    fn component_columns(&self) -> Vec<PyComponentColumnDescriptor> {
        self.schema
            .components
            .iter()
            .map(|c| c.clone().into())
            .collect()
    }

    /// Look up the column descriptor for a specific entity path and component.
    ///
    /// Parameters
    /// ----------
    /// entity_path : str
    ///     The entity path to look up.
    /// component : ComponentLike
    ///     The component to look up.
    ///
    /// Returns
    /// -------
    /// Optional[ComponentColumnDescriptor]
    ///     The column descriptor, if it exists.
    fn column_for(
        &self,
        entity_path: &str,
        component: ComponentLike,
    ) -> Option<PyComponentColumnDescriptor> {
        let entity_path: EntityPath = entity_path.into();

        self.schema.components.iter().find_map(|col| {
            if col.matches(&entity_path, &component.0) {
                Some(col.clone().into())
            } else {
                None
            }
        })
    }

    /// Look up the column descriptor for a specific selector.
    ///
    /// Parameters
    /// ----------
    /// component_column_selector: str | ComponentColumnDescriptor | ComponentColumnSelector
    ///    The selector to look up.
    ///
    ///    String arguments are expected to follow the following format:
    ///    `"<entity_path>:<component_name>"`
    pub fn column_for_selector(
        &self,
        component_column_selector: AnyComponentColumn,
    ) -> PyResult<PyComponentColumnDescriptor> {
        match component_column_selector {
            AnyComponentColumn::Name(name) => self.resolve_component_column_selector(
                &ComponentColumnSelector::from_str(&name).map_err(to_py_err)?,
            ),

            AnyComponentColumn::ComponentDescriptor(desc) => Ok(desc),

            AnyComponentColumn::ComponentSelector(selector) => {
                self.resolve_component_column_selector(&selector.0)
            }
        }
    }
}

impl PySchema {
    pub fn resolve_component_column_selector(
        &self,
        column_selector: &ComponentColumnSelector,
    ) -> PyResult<PyComponentColumnDescriptor> {
        let desc = self
            .schema
            .resolve_component_column_selector(column_selector)
            .map_err(to_py_err)?;

        Ok(PyComponentColumnDescriptor(desc.clone()))
    }
}

/// A single Rerun recording.
///
/// This can be loaded from an RRD file using [`load_recording()`][rerun.dataframe.load_recording].
///
/// A recording is a collection of data that was logged to Rerun. This data is organized
/// as a column for each index (timeline) and each entity/component pair that was logged.
///
/// You can examine the [`.schema()`][rerun.dataframe.Recording.schema] of the recording to see
/// what data is available, or create a [`RecordingView`][rerun.dataframe.RecordingView] to
/// to retrieve the data.
#[pyclass(name = "Recording")]
pub struct PyRecording {
    pub(crate) store: ChunkStoreHandle,
    pub(crate) cache: re_dataframe::QueryCacheHandle,
}

#[derive(Clone)]
pub enum PyRecordingHandle {
    Local(std::sync::Arc<Py<PyRecording>>),
    // TODO(rerun-io/dataplatform#405): interface with remote data needs to be reimplemented
    //Remote(std::sync::Arc<Py<PyRemoteRecording>>),
}

/// A view of a recording restricted to a given index, containing a specific set of entities and components.
///
/// See [`Recording.view(…)`][rerun.dataframe.Recording.view] for details on how to create a `RecordingView`.
///
/// Note: `RecordingView` APIs never mutate the underlying view. Instead, they
/// always return new views with the requested modifications applied.
///
/// The view will only contain a single row for each unique value of the index
/// that is associated with a component column that was included in the view.
/// Component columns that are not included via the view contents will not
/// impact the rows that make up the view. If the same entity / component pair
/// was logged to a given index multiple times, only the most recent row will be
/// included in the view, as determined by the `row_id` column. This will
/// generally be the last value logged, as row_ids are guaranteed to be
/// monotonically increasing when data is sent from a single process.
#[pyclass(name = "RecordingView")]
#[derive(Clone)]
pub struct PyRecordingView {
    pub(crate) recording: PyRecordingHandle,

    pub(crate) query_expression: QueryExpression,
}

impl PyRecordingView {
    fn select_args(
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<Option<Vec<ColumnSelector>>> {
        // Coerce the arguments into a list of `ColumnSelector`s
        let args: Vec<AnyColumn> = args
            .iter()
            .map(|arg| arg.extract::<AnyColumn>())
            .collect::<PyResult<_>>()?;

        if columns.is_some() && !args.is_empty() {
            return Err(PyValueError::new_err(
                "Cannot specify both `columns` and `args` in `select`.",
            ));
        }

        let columns = columns.or(if !args.is_empty() { Some(args) } else { None });

        columns
            .map(|cols| {
                cols.into_iter()
                    .map(|col| col.into_selector())
                    .collect::<PyResult<_>>()
            })
            .transpose()
    }
}

/// A view of a recording restricted to a given index, containing a specific set of entities and components.
///
/// Can only be created by calling `view(...)` on a `Recording`.
///
/// The only type of index currently supported is the name of a timeline.
///
/// The view will only contain a single row for each unique value of the index. If the same entity / component pair
/// was logged to a given index multiple times, only the most recent row will be included in the view, as determined
/// by the `row_id` column. This will generally be the last value logged, as row_ids are guaranteed to be monotonically
/// increasing when data is sent from a single process.
#[pymethods]
impl PyRecordingView {
    /// The schema describing all the columns available in the view.
    ///
    /// This schema will only contain the columns that are included in the view via
    /// the view contents.
    fn schema(&self, py: Python<'_>) -> PySchema {
        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed: PyRef<'_, PyRecording> = recording.borrow(py);
                let engine = borrowed.engine();

                let mut query_expression = self.query_expression.clone();
                query_expression.selection = None;

                let query_handle = engine.query(query_expression);

                PySchema {
                    schema: query_handle.view_contents().clone(),
                }
            }
        }
    }

    /// Select the columns from the view.
    ///
    /// If no columns are provided, all available columns will be included in
    /// the output.
    ///
    /// The selected columns do not change the rows that are included in the
    /// view. The rows are determined by the index values and the components
    /// that were included in the view contents, or can be overridden with
    /// [`.using_index_values()`][rerun.dataframe.RecordingView.using_index_values].
    ///
    /// If a column was not provided with data for a given row, it will be
    /// `null` in the output.
    ///
    /// The output is a [`pyarrow.RecordBatchReader`][] that can be used to read
    /// out the data.
    ///
    /// Parameters
    /// ----------
    /// *args : AnyColumn
    ///     The columns to select.
    /// columns : Optional[Sequence[AnyColumn]], optional
    ///     Alternatively the columns to select can be provided as a sequence.
    ///
    /// Returns
    /// -------
    /// pa.RecordBatchReader
    ///     A reader that can be used to read out the selected data.
    #[pyo3(signature = (
        *args,
        columns = None
    ))]
    fn select(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let mut query_expression = self.query_expression.clone();
        query_expression.selection = Self::select_args(args, columns)?;

        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed = recording.borrow(py);
                let engine = borrowed.engine();

                let query_handle = engine.query(query_expression);

                // If the only contents found are static, we might need to warn the user since
                // this means we won't naturally have any rows in the result.
                let available_data_columns = &query_handle.view_contents().components;

                // We only consider all contents static if there at least some columns
                let all_contents_are_static = !available_data_columns.is_empty()
                    && available_data_columns.iter().all(|c| c.is_static());

                // Additionally, we only want to warn if the user actually tried to select some
                // of the static columns. Otherwise the fact that there are no results shouldn't
                // be surprising.
                let selected_data_columns = query_handle
                    .selected_contents()
                    .iter()
                    .map(|(_, col)| col)
                    .filter(|c| matches!(c, ColumnDescriptor::Component(_)))
                    .collect::<Vec<_>>();

                let any_selected_data_is_static =
                    selected_data_columns.iter().any(|c| c.is_static());

                if self.query_expression.using_index_values.is_none()
                    && all_contents_are_static
                    && any_selected_data_is_static
                {
                    py_rerun_warn(c"RecordingView::select: tried to select static data, but no non-static contents generated an index value on this timeline. No results will be returned. Either include non-static data or consider using `select_static()` instead.")?;
                }

                let schema = query_handle.schema().clone();

                let reader =
                    RecordBatchIterator::new(query_handle.into_batch_iter().map(Ok), schema);
                Ok(PyArrowType(Box::new(reader)))
            }
        }
    }

    /// Select only the static columns from the view.
    ///
    /// Because static data has no associated index values it does not cause a
    /// row to be generated in the output. If your view only contains static data
    /// this method allows you to select it without needing to provide index values.
    ///
    /// This method will always return a single row.
    ///
    /// Any non-static columns that are included in the selection will generate a warning
    /// and produce empty columns.
    ///
    ///
    /// Parameters
    /// ----------
    /// *args : AnyColumn
    ///     The columns to select.
    /// columns : Optional[Sequence[AnyColumn]], optional
    ///     Alternatively the columns to select can be provided as a sequence.
    ///
    /// Returns
    /// -------
    /// pa.RecordBatchReader
    ///     A reader that can be used to read out the selected data.
    #[pyo3(signature = (
        *args,
        columns = None
    ))]
    fn select_static(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let mut query_expression = self.query_expression.clone();
        // This is a static selection, so we clear the filtered index
        query_expression.filtered_index = None;

        // If no columns provided, select all static columns
        let static_columns = Self::select_args(args, columns)
            .transpose()
            .unwrap_or_else(|| {
                Ok(self
                    .schema(py)
                    .schema
                    .components
                    .iter()
                    .filter(|col| col.is_static())
                    .map(|col| ColumnDescriptor::Component(col.clone()).into())
                    .collect())
            })?;

        query_expression.selection = Some(static_columns);

        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed = recording.borrow(py);
                let engine = borrowed.engine();

                let query_handle = engine.query(query_expression);

                let non_static_cols = query_handle
                    .selected_contents()
                    .iter()
                    .filter(|(_, col)| !col.is_static())
                    .collect::<Vec<_>>();

                if !non_static_cols.is_empty() {
                    return Err(PyValueError::new_err(format!(
                        "Static selection resulted in non-static columns: {non_static_cols:?}",
                    )));
                }

                let schema = query_handle.schema().clone();

                let reader =
                    RecordBatchIterator::new(query_handle.into_batch_iter().map(Ok), schema);

                Ok(PyArrowType(Box::new(reader)))
            }
        }
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index sequence numbers.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a sequential index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_sequence(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this check? If so, how can we accomplish it?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Sequence => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not a sequence.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = if let Ok(seq) = re_chunk::TimeInt::try_from(start) {
            seq
        } else {
            re_log::error!(
                illegal_value = start,
                new_value = re_chunk::TimeInt::MIN.as_i64(),
                "set_time_sequence() called with illegal value - clamped to minimum legal value"
            );
            re_chunk::TimeInt::MIN
        };

        let end = if let Ok(seq) = re_chunk::TimeInt::try_from(end) {
            seq
        } else {
            re_log::error!(
                illegal_value = end,
                new_value = re_chunk::TimeInt::MAX.as_i64(),
                "set_time_sequence() called with illegal value - clamped to maximum legal value"
            );
            re_chunk::TimeInt::MAX
        };

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index values expressed as seconds.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a temporal index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_secs(&self, start: f64, end: f64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this check? If so, how can we accomplish it?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not temporal.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_log_types::Timestamp::from_secs_since_epoch(start);
        let end = re_log_types::Timestamp::from_secs_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    /// DEPRECATED: Renamed to `filter_range_secs`.
    #[deprecated(since = "0.23.0", note = "Renamed to `filter_range_secs`")]
    fn filter_range_seconds(&self, start: f64, end: f64) -> PyResult<Self> {
        self.filter_range_secs(start, end)
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index values expressed as nanoseconds.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a temporal index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_nanos(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not temporal.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_log_types::Timestamp::from_nanos_since_epoch(start);
        let end = re_log_types::Timestamp::from_nanos_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data at the provided index values.
    ///
    /// The index values returned will be the intersection between the provided values and the
    /// original index values.
    ///
    /// This requires index values to be a precise match. Index values in Rerun are
    /// represented as i64 sequence counts or nanoseconds. This API does not expose an interface
    /// in floating point seconds, as the numerical conversion would risk false mismatches.
    ///
    /// Parameters
    /// ----------
    /// values : IndexValuesLike
    ///     The index values to filter by.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data at the specified index values.
    ///
    ///     The original view will not be modified.
    fn filter_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include rows where the given component column is not null.
    ///
    /// This corresponds to rows for index values where this component was provided to Rerun explicitly
    /// via `.log()` or `.send_columns()`.
    ///
    /// Parameters
    /// ----------
    /// column : AnyComponentColumn
    ///     The component column to filter by.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data where the specified component column is not null.
    ///
    ///     The original view will not be modified.
    fn filter_is_not_null(&self, column: AnyComponentColumn) -> PyResult<Self> {
        let column = column.into_selector();

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_is_not_null = Some(column?);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Replace the index in the view with the provided values.
    ///
    /// The output view will always have the same number of rows as the provided values, even if
    /// those rows are empty. Use with [`.fill_latest_at()`][rerun.dataframe.RecordingView.fill_latest_at]
    /// to populate these rows with the most recent data.
    ///
    /// This requires index values to be a precise match. Index values in Rerun are
    /// represented as i64 sequence counts or nanoseconds. This API does not expose an interface
    /// in floating point seconds, as the numerical conversion would risk false mismatches.
    ///
    /// Parameters
    /// ----------
    /// values : IndexValuesLike
    ///     The index values to use.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing the provided index values.
    ///
    ///     The original view will not be modified.
    fn using_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.using_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Populate any null values in a row with the latest valid data according to the index.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view with the null values filled in.
    ///
    ///     The original view will not be modified.
    fn fill_latest_at(&self) -> Self {
        let mut query_expression = self.query_expression.clone();
        query_expression.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;

        Self {
            recording: self.recording.clone(),
            query_expression,
        }
    }
}

impl PyRecording {
    fn engine(&self) -> QueryEngine<StorageEngine> {
        // Safety: this is all happening in the context of a python client using the dataframe API,
        // there is no reason to worry about handle leakage whatsoever.
        #[allow(unsafe_code)]
        let engine = unsafe { StorageEngine::new(self.store.clone(), self.cache.clone()) };

        QueryEngine { engine }
    }

    fn find_best_component(&self, entity_path: &EntityPath, component_name: &str) -> ComponentName {
        let selector = ComponentColumnSelector {
            entity_path: entity_path.clone(),
            component_name: component_name.into(),
        };

        self.store
            .read()
            .resolve_component_selector(&selector)
            .component_name
    }

    /// Convert a `ViewContentsLike` into a `ViewContentsSelector`.
    ///
    /// ```python
    /// ViewContentsLike = Union[str, Dict[str, Union[ComponentLike, Sequence[ComponentLike]]]]
    /// ```
    ///
    /// We cant do this with the normal `FromPyObject` mechanisms because we want access to the
    /// `QueryEngine` to resolve the entity paths.
    fn extract_contents_expr(
        &self,
        expr: Bound<'_, PyAny>,
    ) -> PyResult<re_chunk_store::ViewContentsSelector> {
        let engine = self.engine();

        if let Ok(expr) = expr.extract::<String>() {
            // `str`

            let path_filter =
                EntityPathFilter::parse_strict(&expr)
                    .map_err(|err| {
                        PyValueError::new_err(format!(
                            "Could not interpret `contents` as a ViewContentsLike. Failed to parse {expr}: {err}.",
                        ))
                    })?;

            let contents = engine
                .iter_entity_paths_sorted(&path_filter)
                .map(|p| (p, None))
                .collect();

            Ok(contents)
        } else if let Ok(dict) = expr.downcast::<PyDict>() {
            // `Union[ComponentLike, Sequence[ComponentLike]]]`

            let mut contents = ViewContentsSelector::default();

            for (key, value) in dict {
                let key = key.extract::<String>().map_err(|_err| {
                    PyTypeError::new_err(
                        format!("Could not interpret `contents` as a ViewContentsLike. Key: {key} is not a path expression."),
                    )
                })?;

                let path_filter = EntityPathFilter::parse_strict(&key).map_err(|err| {
                    PyValueError::new_err(format!(
                        "Could not interpret `contents` as a ViewContentsLike. Failed to parse {key}: {err}.",
                    ))
                })?;

                let component_strs: BTreeSet<String> = if let Ok(component) =
                    value.extract::<ComponentLike>()
                {
                    std::iter::once(component.0).collect()
                } else if let Ok(components) = value.extract::<Vec<ComponentLike>>() {
                    components.into_iter().map(|c| c.0).collect()
                } else {
                    return Err(PyTypeError::new_err(
                            format!("Could not interpret `contents` as a ViewContentsLike. Value: {value} is not a ComponentLike or Sequence[ComponentLike]."),
                        ));
                };

                contents.append(
                    &mut engine
                        .iter_entity_paths_sorted(&path_filter)
                        .map(|entity_path| {
                            let components = component_strs
                                .iter()
                                .map(|component_name| {
                                    self.find_best_component(&entity_path, component_name)
                                })
                                .collect();
                            (entity_path, Some(components))
                        })
                        .collect(),
                );
            }

            Ok(contents)
        } else {
            return Err(PyTypeError::new_err(
                "Could not interpret `contents` as a ViewContentsLike. Top-level type must be a string or a dictionary.",
            ));
        }
    }
}

#[pymethods]
impl PyRecording {
    /// The schema describing all the columns available in the recording.
    fn schema(&self) -> PySchema {
        PySchema {
            schema: self.store.read().schema(),
        }
    }

    #[allow(rustdoc::private_doc_tests, rustdoc::invalid_rust_codeblocks)]
    /// Create a [`RecordingView`][rerun.dataframe.RecordingView] of the recording according to a particular index and content specification.
    ///
    /// The only type of index currently supported is the name of a timeline.
    ///
    /// The view will only contain a single row for each unique value of the index
    /// that is associated with a component column that was included in the view.
    /// Component columns that are not included via the view contents will not
    /// impact the rows that make up the view. If the same entity / component pair
    /// was logged to a given index multiple times, only the most recent row will be
    /// included in the view, as determined by the `row_id` column. This will
    /// generally be the last value logged, as row_ids are guaranteed to be
    /// monotonically increasing when data is sent from a single process.
    ///
    /// Parameters
    /// ----------
    /// index : str
    ///     The index to use for the view. This is typically a timeline name.
    /// contents : ViewContentsLike
    ///     The content specification for the view.
    ///
    ///     This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
    ///     specifying multiple content-expressions and a respective list of components to select within
    ///     that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
    /// include_semantically_empty_columns : bool, optional
    ///     Whether to include columns that are semantically empty, by default `False`.
    ///
    ///     Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
    /// include_indicator_columns : bool, optional
    ///     Whether to include indicator columns, by default `False`.
    ///
    ///     Indicator columns are components used to represent the presence of an archetype within an entity.
    /// include_tombstone_columns : bool, optional
    ///     Whether to include tombstone columns, by default `False`.
    ///
    ///     Tombstone columns are components used to represent clears. However, even without the clear
    ///     tombstone columns, the view will still apply the clear semantics when resolving row contents.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     The view of the recording.
    ///
    /// Examples
    /// --------
    /// All the data in the recording on the timeline "my_index":
    /// ```python
    /// recording.view(index="my_index", contents="/**")
    /// ```
    ///
    /// Just the Position3D components in the "points" entity:
    /// ```python
    /// recording.view(index="my_index", contents={"points": "Position3D"})
    /// ```
    #[allow(clippy::fn_params_excessive_bools)]
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns = false,
        include_indicator_columns = false,
        include_tombstone_columns = false,
    ))]
    fn view(
        slf: Bound<'_, Self>,
        index: &str,
        contents: Bound<'_, PyAny>,
        include_semantically_empty_columns: bool,
        include_indicator_columns: bool,
        include_tombstone_columns: bool,
    ) -> PyResult<PyRecordingView> {
        let borrowed_self = slf.borrow();

        // Look up the type of the timeline
        let selector = TimeColumnSelector::from(index);

        let time_column = borrowed_self.store.read().resolve_time_selector(&selector);

        let contents = borrowed_self.extract_contents_expr(contents)?;

        let query = QueryExpression {
            view_contents: Some(contents),
            include_semantically_empty_columns,
            include_indicator_columns,
            include_tombstone_columns,
            filtered_index: Some(*time_column.timeline().name()),
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_is_not_null: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            selection: None,
        };

        let recording = slf.unbind();

        Ok(PyRecordingView {
            recording: PyRecordingHandle::Local(std::sync::Arc::new(recording)),
            query_expression: query,
        })
    }

    /// The recording ID of the recording.
    fn recording_id(&self) -> String {
        self.store.read().id().as_str().to_owned()
    }

    /// The application ID of the recording.
    fn application_id(&self) -> PyResult<String> {
        Ok(self
            .store
            .read()
            .info()
            .ok_or(PyValueError::new_err(
                "Recording is missing application id.",
            ))?
            .application_id
            .as_str()
            .to_owned())
    }
}

/// An archive loaded from an RRD.
///
/// RRD archives may include 1 or more recordings or blueprints.
#[pyclass(frozen, name = "RRDArchive")]
#[derive(Clone)]
pub struct PyRRDArchive {
    pub datasets: BTreeMap<StoreId, ChunkStoreHandle>,
}

#[pymethods]
impl PyRRDArchive {
    /// The number of recordings in the archive.
    fn num_recordings(&self) -> usize {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .count()
    }

    /// All the recordings in the archive.
    // TODO(jleibs): This should return an iterator
    fn all_recordings(&self) -> Vec<PyRecording> {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .map(|(_, store)| {
                let cache = re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(
                    store.clone(),
                ));
                PyRecording {
                    store: store.clone(),
                    cache,
                }
            })
            .collect()
    }
}

/// Load a single recording from an RRD file.
///
/// Will raise a `ValueError` if the file does not contain exactly one recording.
///
/// Parameters
/// ----------
/// path_to_rrd : str | os.PathLike[str]
///     The path to the file to load.
///
/// Returns
/// -------
/// Recording
///     The loaded recording.
#[pyfunction]
pub fn load_recording(path_to_rrd: std::path::PathBuf) -> PyResult<PyRecording> {
    let archive = load_archive(path_to_rrd)?;

    let num_recordings = archive.num_recordings();

    if num_recordings != 1 {
        return Err(PyValueError::new_err(format!(
            "Expected exactly one recording in the archive, but found {num_recordings}",
        )));
    }

    if let Some(recording) = archive.all_recordings().into_iter().next() {
        Ok(recording)
    } else {
        Err(PyValueError::new_err(
            "Expected exactly one recording in the archive, but found none.",
        ))
    }
}

/// Load a rerun archive from an RRD file.
///
/// Parameters
/// ----------
/// path_to_rrd : str | os.PathLike[str]
///     The path to the file to load.
///
/// Returns
/// -------
/// RRDArchive
///     The loaded archive.
#[pyfunction]
pub fn load_archive(path_to_rrd: std::path::PathBuf) -> PyResult<PyRRDArchive> {
    let stores = ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .into_iter()
        .map(|(store_id, store)| (store_id, ChunkStoreHandle::new(store)))
        .collect();

    let archive = PyRRDArchive { datasets: stores };

    Ok(archive)
}

#[pyclass(frozen, name = "DataFusionTable")]
pub struct PyDataFusionTable {
    pub provider: Arc<dyn TableProvider + Send>,
    pub name: String,
    pub client: Py<PyCatalogClient>,
}

#[pymethods]
impl PyDataFusionTable {
    /// Returns a DataFusion table provider capsule.
    fn __datafusion_table_provider__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let provider = FFI_TableProvider::new(Arc::clone(&self.provider), false, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }

    /// Register this view to the global DataFusion context and return a DataFrame.
    fn df(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();

        let client = self_.client.borrow(py);

        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);

        drop(client);

        let name = self_.name.clone();

        // We're fine with this failing.
        ctx.call_method1("deregister_table", (name.clone(),))?;

        ctx.call_method1("register_table_provider", (name.clone(), self_))?;

        let df = ctx.call_method1("table", (name.clone(),))?;

        Ok(df)
    }

    /// Name of this table.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
}
