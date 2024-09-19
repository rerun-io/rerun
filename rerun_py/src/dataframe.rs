#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::collections::BTreeMap;

use arrow::{array::RecordBatch, pyarrow::PyArrowType};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor,
    ComponentColumnSelector, ControlColumnDescriptor, ControlColumnSelector, RangeQueryExpression,
    TimeColumnDescriptor, TimeColumnSelector, VersionPolicy,
};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange, TimeType};
use re_sdk::{EntityPath, Loggable as _, StoreId, StoreKind, Timeline};

/// Register the `rerun.dataframe` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchema>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyDataset>()?;
    m.add_class::<PyControlColumnDescriptor>()?;
    m.add_class::<PyControlColumnSelector>()?;
    m.add_class::<PyTimeColumnDescriptor>()?;
    m.add_class::<PyTimeColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;
    m.add_class::<PyTimeRange>()?;

    m.add_function(wrap_pyfunction!(crate::dataframe::load_archive, m)?)?;
    m.add_function(wrap_pyfunction!(crate::dataframe::load_recording, m)?)?;

    Ok(())
}

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

#[pyclass(frozen, name = "TimeRange")]
#[derive(Clone)]
pub struct PyTimeRange {
    time_type: Option<TimeType>,
    range: ResolvedTimeRange,
}

#[pymethods]
impl PyTimeRange {
    #[staticmethod]
    fn everything() -> Self {
        let time_type = None;
        let range = ResolvedTimeRange::EVERYTHING;

        Self { time_type, range }
    }

    #[staticmethod]
    fn seconds(from: f64, to: f64) -> Self {
        let time_type = Some(TimeType::Time);

        let from = re_sdk::Time::from_seconds_since_epoch(from);
        let to = re_sdk::Time::from_seconds_since_epoch(to);

        let range = ResolvedTimeRange::new(from, to);

        Self { time_type, range }
    }

    #[staticmethod]
    fn nanos(from: i64, to: i64) -> Self {
        let time_type = Some(TimeType::Time);

        let from = re_sdk::Time::from_ns_since_epoch(from);
        let to = re_sdk::Time::from_ns_since_epoch(to);

        let range = ResolvedTimeRange::new(from, to);

        Self { time_type, range }
    }

    #[staticmethod]
    fn sequence(from: i64, to: i64) -> Self {
        let time_type = Some(TimeType::Sequence);

        let from = if let Ok(seq) = re_chunk::TimeInt::try_from(from) {
            seq
        } else {
            re_log::error!(
                illegal_value = from,
                new_value = re_chunk::TimeInt::MIN.as_i64(),
                "set_time_sequence() called with illegal value - clamped to minimum legal value"
            );
            re_chunk::TimeInt::MIN
        };

        let to = if let Ok(seq) = re_chunk::TimeInt::try_from(to) {
            seq
        } else {
            re_log::error!(
                illegal_value = to,
                new_value = re_chunk::TimeInt::MAX.as_i64(),
                "set_time_sequence() called with illegal value - clamped to maximum legal value"
            );
            re_chunk::TimeInt::MAX
        };

        let range = ResolvedTimeRange::new(from, to);

        Self { time_type, range }
    }
}

#[pyclass(frozen, name = "Dataset")]
#[derive(Clone)]
pub struct PyDataset {
    pub store: ChunkStore,
}

#[pymethods]
impl PyDataset {
    fn schema(&self) -> PySchema {
        PySchema {
            schema: self.store.schema(),
        }
    }

    fn range_query(
        &self,
        expr: &str,
        timeline: &str,
        time_range: PyTimeRange,
        pov: AnyComponentColumn,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Vec<RecordBatch>>> {
        // TODO(jleibs): Move this ctx into PyChunkStore?
        let cache = re_dataframe::external::re_query::Caches::new(&self.store);
        let engine = QueryEngine {
            store: &self.store,
            cache: &cache,
        };

        let timeline = if let Some(time_type) = time_range.time_type {
            Timeline::new(timeline, time_type)
        } else {
            // Look up the type of the timeline
            let selector = TimeColumnSelector {
                timeline: timeline.into(),
            };
            let resolved = self.store.resolve_time_selector(&selector);

            resolved.timeline
        };

        let query = RangeQueryExpression {
            entity_path_filter: std::convert::TryInto::<EntityPathFilter>::try_into(expr)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
            timeline,
            time_range: time_range.range,
            pov: pov.into_selector(),
        };

        let columns = columns.map(|cols| cols.into_iter().map(|col| col.into_selector()).collect());

        let query_handle = engine.range(&query, columns);

        let batches: Result<Vec<_>, _> = query_handle
            .into_iter()
            .map(|batch| batch.try_to_arrow_record_batch())
            .collect();

        let batches = batches.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(PyArrowType(batches))
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
    fn all_recordings(&self) -> Vec<PyDataset> {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .map(|(_, store)| PyDataset {
                store: store.clone(),
            })
            .collect()
    }
}

#[pyfunction]
pub fn load_recording(path_to_rrd: String) -> PyResult<PyDataset> {
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
