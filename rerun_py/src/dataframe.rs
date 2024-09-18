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
    ChunkStore, ChunkStoreConfig, ColumnDescriptor, ComponentColumnDescriptor,
    ControlColumnDescriptor, RangeQueryExpression, TimeColumnDescriptor, VersionPolicy,
};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange};
use re_sdk::{EntityPath, StoreId, StoreKind, Timeline};

/// Register the `rerun.dataframe` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySchema>()?;

    m.add_class::<PyRRDArchive>()?;
    m.add_class::<PyDataset>()?;
    m.add_class::<PyControlColumnDescriptor>()?;
    m.add_class::<PyTimeColumnDescriptor>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;

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

#[derive(FromPyObject)]
enum AnyColumn {
    #[pyo3(transparent, annotation = "control")]
    Control(PyControlColumnDescriptor),
    #[pyo3(transparent, annotation = "time")]
    Time(PyTimeColumnDescriptor),
    #[pyo3(transparent, annotation = "component")]
    Component(PyComponentColumnDescriptor),
}

impl AnyColumn {
    fn into_column(self) -> ColumnDescriptor {
        match self {
            Self::Control(col) => ColumnDescriptor::Control(col.0),
            Self::Time(col) => ColumnDescriptor::Time(col.0),
            Self::Component(col) => ColumnDescriptor::Component(col.0),
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
        component: &Bound<'_, PyAny>, // str | type[ComponentMixin]
    ) -> PyResult<Option<PyComponentColumnDescriptor>> {
        let entity_path: EntityPath = entity_path.into();
        let component_name: re_chunk::ComponentName;

        if let Ok(component_str) = component.extract::<String>() {
            component_name = component_str.into();
        } else if let Ok(component_str) = component
            .getattr("_BATCH_TYPE")
            .and_then(|batch_type| batch_type.getattr("_ARROW_TYPE"))
            .and_then(|arrow_type| arrow_type.getattr("_TYPE_NAME"))
            .and_then(|type_name| type_name.extract::<String>())
        {
            component_name = component_str.into();
        } else {
            return Err(PyTypeError::new_err(
                "Input to parameter `component` must be a string or Component class.",
            ));
        }

        Ok(self.schema.iter().find_map(|col| {
            if let ColumnDescriptor::Component(col) = col {
                if col.matches(&entity_path, &component_name) {
                    return Some(col.clone().into());
                }
            }
            None
        }))
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
        pov: PyComponentColumnDescriptor,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Vec<RecordBatch>>> {
        // TODO(jleibs): Move this ctx into PyChunkStore?
        let cache = re_dataframe::external::re_query::Caches::new(&self.store);
        let engine = QueryEngine {
            store: &self.store,
            cache: &cache,
        };

        // TODO(jleibs): Move to arguments
        let timeline = Timeline::log_tick();
        let time_range = ResolvedTimeRange::EVERYTHING;

        let query = RangeQueryExpression {
            entity_path_filter: std::convert::TryInto::<EntityPathFilter>::try_into(expr)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
            timeline,
            time_range,
            pov: pov.0,
        };

        let columns = columns.map(|cols| cols.into_iter().map(|col| col.into_column()).collect());

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
