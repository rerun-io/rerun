#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::collections::{BTreeMap, BTreeSet};

use arrow::{array::RecordBatch, pyarrow::PyArrowType};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyDict,
};

use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor,
    ComponentColumnSelector, ControlColumnDescriptor, ControlColumnSelector, QueryExpression2,
    SparseFillStrategy, TimeColumnDescriptor, TimeColumnSelector, VersionPolicy,
    ViewContentsSelector,
};
use re_dataframe2::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange, TimeType};
use re_sdk::{ComponentName, EntityPath, Loggable as _, StoreId, StoreKind};

/// Register the `rerun.dataframe` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchema>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyRecording>()?;
    m.add_class::<PyControlColumnDescriptor>()?;
    m.add_class::<PyControlColumnSelector>()?;
    m.add_class::<PyTimeColumnDescriptor>()?;
    m.add_class::<PyTimeColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;
    m.add_class::<PyRecordingView>()?;

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}

/// Python binding for [`ControlColumnDescriptor`]`
#[pyclass(frozen, name = "ControlColumnDescriptor")]
#[derive(Clone)]
struct PyControlColumnDescriptor(ControlColumnDescriptor);

#[pymethods]
impl PyControlColumnDescriptor {
    fn __repr__(&self) -> String {
        format!("Ctrl({})", self.0.component_name.short_name())
    }
}

impl From<ControlColumnDescriptor> for PyControlColumnDescriptor {
    fn from(desc: ControlColumnDescriptor) -> Self {
        Self(desc)
    }
}

/// Python binding for [`ControlColumnSelector`]`
#[pyclass(frozen, name = "ControlColumnSelector")]
#[derive(Clone)]
struct PyControlColumnSelector(ControlColumnSelector);

#[pymethods]
impl PyControlColumnSelector {
    #[staticmethod]
    fn row_id() -> Self {
        Self(ControlColumnSelector {
            component: re_chunk::RowId::name(),
        })
    }

    fn __repr__(&self) -> String {
        format!("Ctrl({})", self.0.component.short_name())
    }
}

/// Python binding for [`TimeColumnDescriptor`]`
#[pyclass(frozen, name = "TimeColumnDescriptor")]
#[derive(Clone)]
struct PyTimeColumnDescriptor(TimeColumnDescriptor);

#[pymethods]
impl PyTimeColumnDescriptor {
    fn __repr__(&self) -> String {
        format!("Time({})", self.0.timeline.name())
    }
}

impl From<TimeColumnDescriptor> for PyTimeColumnDescriptor {
    fn from(desc: TimeColumnDescriptor) -> Self {
        Self(desc)
    }
}

/// Python binding for [`TimeColumnSelector`]`
#[pyclass(frozen, name = "TimeColumnSelector")]
#[derive(Clone)]
struct PyTimeColumnSelector(TimeColumnSelector);

#[pymethods]
impl PyTimeColumnSelector {
    #[new]
    fn new(timeline: &str) -> Self {
        Self(TimeColumnSelector {
            timeline: timeline.into(),
        })
    }

    fn __repr__(&self) -> String {
        format!("Time({})", self.0.timeline)
    }
}

/// Python binding for [`ComponentColumnDescriptor`]`

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
    fn with_dictionary_encoding(&self) -> Self {
        Self(
            self.0
                .clone()
                .with_join_encoding(re_chunk_store::JoinEncoding::DictionaryEncode),
        )
    }

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
}

impl From<PyComponentColumnDescriptor> for ComponentColumnDescriptor {
    fn from(desc: PyComponentColumnDescriptor) -> Self {
        desc.0
    }
}

/// Python binding for [`ComponentColumnSelector`]`
#[pyclass(frozen, name = "ComponentColumnSelector")]
#[derive(Clone)]
struct PyComponentColumnSelector(ComponentColumnSelector);

#[pymethods]
impl PyComponentColumnSelector {
    #[new]
    fn new(entity_path: &str, component_name: ComponentLike) -> Self {
        Self(ComponentColumnSelector {
            entity_path: entity_path.into(),
            component: component_name.0,
            join_encoding: Default::default(),
        })
    }

    fn with_dictionary_encoding(&self) -> Self {
        Self(
            self.0
                .clone()
                .with_join_encoding(re_chunk_store::JoinEncoding::DictionaryEncode),
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Component({}:{})",
            self.0.entity_path,
            self.0.component.short_name()
        )
    }
}

/// Python binding for [`AnyColumn`] type-alias.
#[derive(FromPyObject)]
enum AnyColumn {
    #[pyo3(transparent, annotation = "control_descriptor")]
    ControlDescriptor(PyControlColumnDescriptor),
    #[pyo3(transparent, annotation = "control_selector")]
    ControlSelector(PyControlColumnSelector),
    #[pyo3(transparent, annotation = "time_descriptor")]
    TimeDescriptor(PyTimeColumnDescriptor),
    #[pyo3(transparent, annotation = "time_selector")]
    TimeSelector(PyTimeColumnSelector),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyColumn {
    fn into_selector(self) -> ColumnSelector {
        match self {
            Self::ControlDescriptor(desc) => ColumnDescriptor::Control(desc.0).into(),
            Self::ControlSelector(selector) => selector.0.into(),
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

struct ComponentLike(re_sdk::ComponentName);

impl FromPyObject<'_> for ComponentLike {
    fn extract(component: &PyAny) -> PyResult<Self> {
        if let Ok(component_str) = component.extract::<String>() {
            Ok(Self(component_str.into()))
        } else if let Ok(component_str) = component
            .getattr("_BATCH_TYPE")
            .and_then(|batch_type| batch_type.getattr("_ARROW_TYPE"))
            .and_then(|arrow_type| arrow_type.getattr("_TYPE_NAME"))
            .and_then(|type_name| type_name.extract::<String>())
        {
            Ok(Self(component_str.into()))
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
    // TODO(jleibs): This gets replaced with the new schema object
    pub schema: Vec<ColumnDescriptor>,
}

#[pymethods]
impl PySchema {
    fn control_columns(&self) -> Vec<PyControlColumnDescriptor> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Control(col) = column {
                    Some(col.clone().into())
                } else {
                    None
                }
            })
            .collect()
    }

    fn time_columns(&self) -> Vec<PyTimeColumnDescriptor> {
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
    cache: re_dataframe2::external::re_query::Caches,
}

#[pyclass(name = "RecordingView")]
#[derive(Clone)]
pub struct PyRecordingView {
    recording: Py<PyRecording>,

    query_expression: QueryExpression2,
}

/// A view of a recording on a timeline, containing a specific set of entities and components.
///
/// Can only be created by calling `view(...)` on a `Recording`.
#[pymethods]
impl PyRecordingView {
    fn select(
        &self,
        py: Python<'_>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Vec<RecordBatch>>> {
        let borrowed = self.recording.borrow(py);
        let engine = borrowed.engine();

        let mut query_expression = self.query_expression.clone();
        query_expression.selection =
            columns.map(|cols| cols.into_iter().map(|col| col.into_selector()).collect());

        let query_handle = engine.query(query_expression);

        let batches: Result<Vec<_>, _> = query_handle
            .into_batch_iter()
            .map(|batch| batch.try_to_arrow_record_batch())
            .collect();

        let batches = batches.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(PyArrowType(batches))
    }

    fn filter_range_sequence(&self, start: i64, end: i64) -> PyResult<Self> {
        if self.query_expression.filtered_index.typ() != TimeType::Sequence {
            return Err(PyValueError::new_err(format!(
                "Timeline for {} is not a sequence.",
                self.query_expression.filtered_index.name()
            )));
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
        if self.query_expression.filtered_index.typ() != TimeType::Time {
            return Err(PyValueError::new_err(format!(
                "Timeline for {} is not temporal.",
                self.query_expression.filtered_index.name()
            )));
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
        if self.query_expression.filtered_index.typ() != TimeType::Time {
            return Err(PyValueError::new_err(format!(
                "Timeline for {} is not temporal.",
                self.query_expression.filtered_index.name()
            )));
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
}

impl PyRecording {
    fn engine(&self) -> QueryEngine<'_> {
        QueryEngine {
            store: &self.store,
            cache: &self.cache,
        }
    }

    /// Convert a `ViewContentsLike` into a `ViewContentsSelector`.
    ///
    /// ```python
    /// ViewContentsLike = Union[str, Dict[str, Union[ComponentLike, Sequence[ComponentLike]]]]
    /// ```
    ///
    /// We cant do this with the norma `FromPyObject` mechanisms because we want access to the
    /// `QueryEngine` to resolve the entity paths.
    fn extract_contents_expr(
        &self,
        expr: Bound<'_, PyAny>,
    ) -> PyResult<re_chunk_store::ViewContentsSelector> {
        let engine = self.engine();

        if let Ok(expr) = expr.extract::<String>() {
            // `str`

            let path_filter = EntityPathFilter::parse_strict(&expr, &Default::default())
                .map_err(|err| PyValueError::new_err(err.to_string()))?;

            let contents = engine
                .iter_entity_paths(&path_filter)
                .map(|p| (p, None))
                .collect();

            Ok(contents)
        } else if let Ok(dict) = expr.downcast::<PyDict>() {
            // `Union[ComponentLike, Sequence[ComponentLike]]]`

            let mut contents = ViewContentsSelector::default();
            for (key, value) in dict {
                let key = key.extract::<String>()?;

                let path_filter = EntityPathFilter::parse_strict(&key, &Default::default())
                    .map_err(|err| PyValueError::new_err(err.to_string()))?;

                let components: BTreeSet<ComponentName> =
                    if let Ok(component) = value.extract::<ComponentLike>() {
                        std::iter::once(component.0).collect()
                    } else if let Ok(components) = value.extract::<Vec<ComponentLike>>() {
                        components.into_iter().map(|c| c.0).collect()
                    } else {
                        return Err(PyTypeError::new_err(
                            "ViewContentsLike input must be a string or a list of strings.",
                        ));
                    };

                contents.append(
                    &mut engine
                        .iter_entity_paths(&path_filter)
                        .map(|p| (p, Some(components.clone())))
                        .collect(),
                );
            }

            Ok(contents)
        } else {
            return Err(PyTypeError::new_err("ViewContentsLike input must be..."));
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

    #[pyo3(signature = (
        *,
        timeline,
        contents
    ))]
    fn view(
        slf: Bound<'_, Self>,
        timeline: &str,
        contents: Bound<'_, PyAny>,
    ) -> PyResult<PyRecordingView> {
        let borrowed_self = slf.borrow();

        // Look up the type of the timelin
        let selector = TimeColumnSelector {
            timeline: timeline.into(),
        };

        let timeline = borrowed_self.store.resolve_time_selector(&selector);

        let contents = borrowed_self.extract_contents_expr(contents)?;

        let query = QueryExpression2 {
            view_contents: Some(contents),
            filtered_index: timeline.timeline,
            filtered_index_range: None,
            filtered_index_values: None,
            sampled_index_values: None,
            filtered_point_of_view: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            selection: None,
        };

        let recording = slf.unbind();

        Ok(PyRecordingView {
            recording,
            query_expression: query,
        })
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
                let cache = re_dataframe2::external::re_query::Caches::new(store);
                PyRecording {
                    store: store.clone(),
                    cache,
                }
            })
            .collect()
    }
}

#[pyfunction]
pub fn load_recording(path_to_rrd: String) -> PyResult<PyRecording> {
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
pub fn load_archive(path_to_rrd: String) -> PyResult<PyRRDArchive> {
    let stores =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd, VersionPolicy::Warn)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    let archive = PyRRDArchive { datasets: stores };

    Ok(archive)
}
