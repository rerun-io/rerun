#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::collections::{BTreeMap, BTreeSet};

use arrow::{
    array::{make_array, Array, ArrayData, Int64Array, RecordBatchIterator, RecordBatchReader},
    pyarrow::PyArrowType,
};
use numpy::PyArrayMethods as _;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyDict, PyTuple},
};

use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor,
    ComponentColumnSelector, QueryExpression, SparseFillStrategy, TimeColumnDescriptor,
    TimeColumnSelector, VersionPolicy, ViewContentsSelector,
};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange, TimeType};
use re_sdk::{ComponentName, EntityPath, StoreId, StoreKind};

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

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}

/// Python binding for `IndexColumnDescriptor`
#[pyclass(frozen, name = "IndexColumnDescriptor")]
#[derive(Clone)]
struct PyIndexColumnDescriptor(TimeColumnDescriptor);

#[pymethods]
impl PyIndexColumnDescriptor {
    fn __repr__(&self) -> String {
        format!("Index(timeline:{})", self.0.timeline.name())
    }

    #[getter]
    fn name(&self) -> &str {
        self.0.timeline.name()
    }
}

impl From<TimeColumnDescriptor> for PyIndexColumnDescriptor {
    fn from(desc: TimeColumnDescriptor) -> Self {
        Self(desc)
    }
}

/// Python binding for `IndexColumnSelector`
#[pyclass(frozen, name = "IndexColumnSelector")]
#[derive(Clone)]
struct PyIndexColumnSelector(TimeColumnSelector);

#[pymethods]
impl PyIndexColumnSelector {
    #[new]
    #[pyo3(text_signature = "(self, index)")]
    fn new(index: &str) -> Self {
        Self(TimeColumnSelector {
            timeline: index.into(),
        })
    }

    fn __repr__(&self) -> String {
        format!("Index(timeline:{})", self.0.timeline)
    }

    #[getter]
    fn name(&self) -> &str {
        &self.0.timeline
    }
}

/// Python binding for [`ComponentColumnDescriptor`]

#[pyclass(frozen, name = "ComponentColumnDescriptor")]
#[derive(Clone)]
struct PyComponentColumnDescriptor(ComponentColumnDescriptor);

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

    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    #[getter]
    fn component_name(&self) -> &str {
        &self.0.component_name
    }

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

/// Python binding for [`ComponentColumnSelector`]
#[pyclass(frozen, name = "ComponentColumnSelector")]
#[derive(Clone)]
struct PyComponentColumnSelector(ComponentColumnSelector);

#[pymethods]
impl PyComponentColumnSelector {
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

    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    #[getter]
    fn component_name(&self) -> &str {
        &self.0.component_name
    }
}

/// Python binding for [`AnyColumn`] type-alias.
#[derive(FromPyObject)]
enum AnyColumn {
    #[pyo3(transparent, annotation = "time_descriptor")]
    TimeDescriptor(PyIndexColumnDescriptor),
    #[pyo3(transparent, annotation = "time_selector")]
    TimeSelector(PyIndexColumnSelector),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyColumn {
    fn into_selector(self) -> ColumnSelector {
        match self {
            Self::TimeDescriptor(desc) => ColumnDescriptor::Time(desc.0).into(),
            Self::TimeSelector(selector) => selector.0.into(),
            Self::ComponentDescriptor(desc) => ColumnDescriptor::Component(desc.0).into(),
            Self::ComponentSelector(selector) => selector.0.into(),
        }
    }
}

#[derive(FromPyObject)]
enum AnyComponentColumn {
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyComponentColumn {
    #[allow(dead_code)]
    fn into_selector(self) -> ComponentColumnSelector {
        match self {
            Self::ComponentDescriptor(desc) => desc.0.into(),
            Self::ComponentSelector(selector) => selector.0,
        }
    }
}

#[derive(FromPyObject)]
enum IndexValuesLike<'py> {
    PyArrow(PyArrowType<ArrayData>),
    NumPy(numpy::PyArrayLike1<'py, i64>),

    // Catch all to support ChunkedArray and other types
    #[pyo3(transparent)]
    CatchAll(Bound<'py, PyAny>),
}

impl<'py> IndexValuesLike<'py> {
    fn to_index_values(&self) -> PyResult<BTreeSet<re_chunk_store::TimeInt>> {
        match self {
            Self::PyArrow(array) => {
                let array = make_array(array.0.clone());

                let int_array = array.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
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
                    for chunk in chunks.iter()? {
                        let chunk = chunk?.extract::<PyArrowType<ArrayData>>()?;
                        let array = make_array(chunk.0.clone());

                        let int_array =
                            array.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
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

struct ComponentLike(String);

impl FromPyObject<'_> for ComponentLike {
    fn extract_bound(component: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(component_str) = component.extract::<String>() {
            Ok(Self(component_str))
        } else if let Ok(component_str) = component
            .getattr("_BATCH_TYPE")
            .and_then(|batch_type| batch_type.getattr("_ARROW_TYPE"))
            .and_then(|arrow_type| arrow_type.getattr("_TYPE_NAME"))
            .and_then(|type_name| type_name.extract::<String>())
        {
            Ok(Self(component_str))
        } else {
            return Err(PyTypeError::new_err(
                "ComponentLike input must be a string or Component class.",
            ));
        }
    }
}

#[pyclass(frozen, name = "Schema")]
#[derive(Clone)]
pub struct PySchema {
    pub schema: Vec<ColumnDescriptor>,
}

#[pymethods]
impl PySchema {
    fn index_columns(&self) -> Vec<PyIndexColumnDescriptor> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Time(col) = column {
                    Some(col.clone().into())
                } else {
                    None
                }
            })
            .collect()
    }

    fn component_columns(&self) -> Vec<PyComponentColumnDescriptor> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Component(col) = column {
                    Some(col.clone().into())
                } else {
                    None
                }
            })
            .collect()
    }

    fn column_for(
        &self,
        entity_path: &str,
        component: ComponentLike,
    ) -> Option<PyComponentColumnDescriptor> {
        let entity_path: EntityPath = entity_path.into();

        self.schema.iter().find_map(|col| {
            if let ColumnDescriptor::Component(col) = col {
                if col.matches(&entity_path, &component.0) {
                    return Some(col.clone().into());
                }
            }
            None
        })
    }
}

#[pyclass(name = "Recording")]
pub struct PyRecording {
    store: ChunkStore,
    cache: re_dataframe::QueryCache,
}

#[pyclass(name = "RecordingView")]
#[derive(Clone)]
pub struct PyRecordingView {
    recording: Py<PyRecording>,

    query_expression: QueryExpression,
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

        let columns = columns.or_else(|| if !args.is_empty() { Some(args) } else { None });

        Ok(columns.map(|cols| cols.into_iter().map(|col| col.into_selector()).collect()))
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
    fn schema(&self, py: Python<'_>) -> PySchema {
        let borrowed = self.recording.borrow(py);
        let engine = borrowed.engine();

        let mut query_expression = self.query_expression.clone();
        query_expression.selection = None;

        let query_handle = engine.query(query_expression);

        let contents = query_handle.view_contents();

        PySchema {
            schema: contents.to_vec(),
        }
    }

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
        let borrowed = self.recording.borrow(py);
        let engine = borrowed.engine();

        let mut query_expression = self.query_expression.clone();

        query_expression.selection = Self::select_args(args, columns)?;

        let query_handle = engine.query(query_expression);

        let schema = query_handle.schema();
        let fields: Vec<arrow::datatypes::Field> =
            schema.fields.iter().map(|f| f.clone().into()).collect();
        let metadata = schema.metadata.clone().into_iter().collect();
        let schema = arrow::datatypes::Schema::new(fields).with_metadata(metadata);

        // TODO(jleibs): Need to keep the engine alive
        /*
        let reader = RecordBatchIterator::new(
            query_handle
                .into_batch_iter()
                .map(|batch| batch.try_to_arrow_record_batch()),
            std::sync::Arc::new(schema),
        );
        */
        let batches = query_handle
            .into_batch_iter()
            .map(|batch| batch.try_to_arrow_record_batch())
            .collect::<Vec<_>>();

        let reader = RecordBatchIterator::new(batches.into_iter(), std::sync::Arc::new(schema));

        Ok(PyArrowType(Box::new(reader)))
    }

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
        let borrowed = self.recording.borrow(py);
        let engine = borrowed.engine();

        let mut query_expression = self.query_expression.clone();

        // This is a static selection, so we clear the filtered index
        query_expression.filtered_index = None;

        // If no columns provided, select all static columns
        let static_columns = Self::select_args(args, columns)?.unwrap_or_else(|| {
            self.schema(py)
                .schema
                .iter()
                .filter(|col| col.is_static())
                .map(|col| col.clone().into())
                .collect()
        });

        query_expression.selection = Some(static_columns);

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

        let schema = query_handle.schema();
        let fields: Vec<arrow::datatypes::Field> =
            schema.fields.iter().map(|f| f.clone().into()).collect();
        let metadata = schema.metadata.clone().into_iter().collect();
        let schema = arrow::datatypes::Schema::new(fields).with_metadata(metadata);

        // TODO(jleibs): Need to keep the engine alive
        /*
        let reader = RecordBatchIterator::new(
            query_handle
                .into_batch_iter()
                .map(|batch| batch.try_to_arrow_record_batch()),
            std::sync::Arc::new(schema),
        );
        */
        let batches = query_handle
            .into_batch_iter()
            .map(|batch| batch.try_to_arrow_record_batch())
            .collect::<Vec<_>>();

        let reader = RecordBatchIterator::new(batches.into_iter(), std::sync::Arc::new(schema));

        Ok(PyArrowType(Box::new(reader)))
    }

    fn filter_range_sequence(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            Some(filtered_index) if filtered_index.typ() != TimeType::Sequence => {
                return Err(PyValueError::new_err(format!(
                    "Index for {} is not a sequence.",
                    filtered_index.name()
                )));
            }

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

    fn filter_range_seconds(&self, start: f64, end: f64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
                return Err(PyValueError::new_err(format!(
                    "Index for {} is not temporal.",
                    filtered_index.name()
                )));
            }

            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_sdk::Time::from_seconds_since_epoch(start);
        let end = re_sdk::Time::from_seconds_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    fn filter_range_nanos(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
                return Err(PyValueError::new_err(format!(
                    "Index for {} is not temporal.",
                    filtered_index.name()
                )));
            }

            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_sdk::Time::from_ns_since_epoch(start);
        let end = re_sdk::Time::from_ns_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    fn filter_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    fn filter_is_not_null(&self, column: AnyComponentColumn) -> Self {
        let column = column.into_selector();

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_is_not_null = Some(column);

        Self {
            recording: self.recording.clone(),
            query_expression,
        }
    }

    fn using_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.using_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

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
    fn engine(&self) -> QueryEngine<'_> {
        QueryEngine {
            store: &self.store,
            cache: &self.cache,
        }
    }

    fn find_best_component(&self, entity_path: &EntityPath, component_name: &str) -> ComponentName {
        let selector = ComponentColumnSelector {
            entity_path: entity_path.clone(),
            component_name: component_name.into(),
        };

        self.store
            .resolve_component_selector(&selector)
            .component_name
    }

    /// Convert a `ViewContentsLike` into a `ViewContentsSelector`.
    ///
    /// ```pytholn
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
                EntityPathFilter::parse_strict(&expr, &Default::default()).map_err(|err| {
                    PyValueError::new_err(format!(
                        "Could not interpret `contents` as a ViewContentsLike. Failed to parse {expr}: {err}.",
                    ))
                })?;

            let contents = engine
                .iter_entity_paths(&path_filter)
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

                let path_filter = EntityPathFilter::parse_strict(&key, &Default::default()).map_err(|err| {
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
                        .iter_entity_paths(&path_filter)
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
    fn schema(&self) -> PySchema {
        PySchema {
            schema: self.store.schema(),
        }
    }

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
        let selector = TimeColumnSelector {
            timeline: index.into(),
        };

        let timeline = borrowed_self.store.resolve_time_selector(&selector);

        let contents = borrowed_self.extract_contents_expr(contents)?;

        let query = QueryExpression {
            view_contents: Some(contents),
            include_semantically_empty_columns,
            include_indicator_columns,
            include_tombstone_columns,
            filtered_index: Some(timeline.timeline),
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_is_not_null: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            selection: None,
        };

        let recording = slf.unbind();

        Ok(PyRecordingView {
            recording,
            query_expression: query,
        })
    }

    fn recording_id(&self) -> String {
        self.store.id().as_str().to_owned()
    }

    fn application_id(&self) -> PyResult<String> {
        Ok(self
            .store
            .info()
            .ok_or(PyValueError::new_err(
                "Recording is missing application id.",
            ))?
            .application_id
            .as_str()
            .to_owned())
    }
}

#[pyclass(frozen, name = "RRDArchive")]
#[derive(Clone)]
pub struct PyRRDArchive {
    pub datasets: BTreeMap<StoreId, ChunkStore>,
}

#[pymethods]
impl PyRRDArchive {
    fn num_recordings(&self) -> usize {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .count()
    }

    // TODO(jleibs): This could probably return an iterator
    fn all_recordings(&self) -> Vec<PyRecording> {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .map(|(_, store)| {
                let cache = re_dataframe::external::re_query::Caches::new(store);
                PyRecording {
                    store: store.clone(),
                    cache,
                }
            })
            .collect()
    }
}

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

#[pyfunction]
pub fn load_archive(path_to_rrd: std::path::PathBuf) -> PyResult<PyRRDArchive> {
    let stores =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd, VersionPolicy::Warn)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    let archive = PyRRDArchive { datasets: stores };

    Ok(archive)
}
