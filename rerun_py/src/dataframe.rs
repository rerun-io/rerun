#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::collections::BTreeMap;

use arrow::{array::RecordBatch, pyarrow::PyArrowType};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ColumnDescriptor, ComponentColumnDescriptor,
    ControlColumnDescriptor, RangeQueryExpression, TimeColumnDescriptor, VersionPolicy,
};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange};
use re_sdk::{StoreId, StoreKind, Timeline};

#[pyclass(frozen)]
#[derive(Clone)]
pub struct PyControlColumn {
    pub column: ControlColumnDescriptor,
}

#[pymethods]
impl PyControlColumn {
    fn __repr__(&self) -> String {
        format!("Ctrl({})", self.column.component_name.short_name())
    }
}

#[pyclass(frozen)]
#[derive(Clone)]
pub struct PyTimeColumn {
    pub column: TimeColumnDescriptor,
}

#[pymethods]
impl PyTimeColumn {
    fn __repr__(&self) -> String {
        format!("Time({})", self.column.timeline.name())
    }
}

#[pyclass(frozen)]
#[derive(Clone)]
pub struct PyComponentColumn {
    pub column: ComponentColumnDescriptor,
}

#[pymethods]
impl PyComponentColumn {
    fn __repr__(&self) -> String {
        format!(
            "Component({}:{})",
            self.column.entity_path,
            self.column.component_name.short_name()
        )
    }

    fn matches(&self, entity_path: &str, component_name: &str) -> bool {
        self.column.entity_path == entity_path.into()
            && self.column.component_name == component_name
    }
}

#[pyclass(frozen)]
#[derive(Clone)]
pub struct PySchema {
    // TODO(jleibs): This gets replaced with the new schema object
    pub schema: Vec<ColumnDescriptor>,
}

#[pymethods]
impl PySchema {
    fn control_columns(&self) -> Vec<PyControlColumn> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Control(col) = column {
                    Some(PyControlColumn {
                        column: col.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn time_columns(&self) -> Vec<PyTimeColumn> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Time(col) = column {
                    Some(PyTimeColumn {
                        column: col.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn component_columns(&self) -> Vec<PyComponentColumn> {
        self.schema
            .iter()
            .filter_map(|column| {
                if let ColumnDescriptor::Component(col) = column {
                    Some(PyComponentColumn {
                        column: col.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[pyclass(frozen)]
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
        pov: PyComponentColumn,
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
            pov: pov.column,
        };

        let query_handle = engine.range(&query, None /* columns */);

        let batches: Result<Vec<_>, _> = query_handle
            .into_iter()
            .map(|batch| batch.try_to_arrow_record_batch())
            .collect();

        let batches = batches.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(PyArrowType(batches))
    }
}

#[pyclass(frozen)]
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
pub fn load_rrd(path_to_rrd: String) -> PyResult<PyRRDArchive> {
    let stores =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd, VersionPolicy::Warn)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    let archive = PyRRDArchive { datasets: stores };

    Ok(archive)
}
