//! Methods for handling Arrow datamodel log ingest

use std::borrow::Cow;
use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayData as ArrowArrayData, ArrayRef as ArrowArrayRef,
    FixedSizeListArray as ArrowFixedSizeListArray, Float32Array as ArrowFloat32Array,
    ListArray as ArrowListArray, make_array,
};
use arrow::buffer::OffsetBuffer as ArrowOffsetBuffer;
use arrow::datatypes::{DataType as ArrowDataType, Field as ArrowField};
use arrow::pyarrow::PyArrowType;
use numpy::PyReadonlyArray1;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::{PyAnyMethods as _, PyDict, PyDictMethods as _, PyString};
use pyo3::{Bound, PyAny, PyResult, pyclass, pyfunction, pymethods};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, ChunkError, ChunkId, PendingRow, RowId, TimeColumn, TimelineName};
use re_log_types::TimePoint;
use re_sdk::external::nohash_hasher::IntMap;
use re_sdk::{ComponentDescriptor, EntityPath, Timeline};

use crate::python_bridge::PyComponentDescriptor;

/// An opaque handle to a Rust Arrow array, bypassing PyArrow on the hot path.
///
/// Data stays as `Arc<dyn Array>` on the Rust side. When the Python logging
/// pipeline hands this back to Rust via `array_to_rust`, we just clone the Arc
/// instead of round-tripping through PyArrow's FFI export/import.
#[pyclass(frozen, name = "NativeArrowArray", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
pub struct NativeArrowArray {
    pub(crate) inner: ArrowArrayRef,
}

#[pymethods]
impl NativeArrowArray {
    /// Number of top-level elements (needed by `BaseBatch.__len__`).
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("NativeArrowArray(len={})", self.inner.len())
    }

    /// Escape hatch: export to a real `pyarrow.Array` for cold-path consumers.
    fn to_pyarrow(&self) -> PyArrowType<ArrowArrayData> {
        PyArrowType(self.inner.to_data())
    }
}

/// Perform Python-to-Rust conversion for a `ComponentDescriptor`.
pub fn descriptor_to_rust(component_descr: &Bound<'_, PyAny>) -> PyResult<ComponentDescriptor> {
    // Fast path: if we already have a PyComponentDescriptor, just clone its inner descriptor.
    if let Ok(py_descr) = component_descr.downcast::<PyComponentDescriptor>() {
        return Ok(py_descr.borrow().0.clone());
    }

    // Fallback: extract fields via getattr (for duck-typed descriptors).
    let py = component_descr.py();

    let archetype = component_descr.getattr(pyo3::intern!(py, "archetype"))?;
    let archetype: Option<Cow<'_, str>> = if !archetype.is_none() {
        Some(archetype.extract()?)
    } else {
        None
    };

    let component_type = component_descr.getattr(pyo3::intern!(py, "component_type"))?;
    let component_type: Option<Cow<'_, str>> = if !component_type.is_none() {
        Some(component_type.extract()?)
    } else {
        None
    };

    let component = component_descr.getattr(pyo3::intern!(py, "component"))?;
    let component: Cow<'_, str> = component.extract()?;

    let descr = ComponentDescriptor {
        archetype: archetype.map(|s| s.as_ref().into()),
        component: component.as_ref().into(),
        component_type: component_type.map(|s| s.as_ref().into()),
    };
    descr.sanity_check();
    Ok(descr)
}

/// Perform conversion between a pyarrow array (or NativeArrowArray) to arrow types.
///
/// Fast path: if the object is a `NativeArrowArray`, just clone the inner `Arc`.
/// Fallback: extract via PyArrow FFI.
pub fn array_to_rust(arrow_array: &Bound<'_, PyAny>) -> PyResult<ArrowArrayRef> {
    // Fast path: NativeArrowArray — just clone the Arc, no FFI round-trip.
    if let Ok(native) = arrow_array.downcast::<NativeArrowArray>() {
        return Ok(native.borrow().inner.clone());
    }
    // Fallback: PyArrow FFI
    let py_array: PyArrowType<ArrowArrayData> = arrow_array.extract()?;
    Ok(make_array(py_array.0))
}

/// Build a [`PendingRow`] given a '**kwargs'-style dictionary of component arrays.
pub fn build_row_from_components(
    components_per_descr: &Bound<'_, PyDict>,
    time_point: &TimePoint,
) -> PyResult<PendingRow> {
    // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    // TODO(emilk): move to before we arrow-serialize the data
    let row_id = RowId::new();

    let mut components = IntMap::default();
    for (component_descr, array) in components_per_descr {
        let component_descr = descriptor_to_rust(&component_descr)?;
        let list_array = array_to_rust(&array)?;
        let batch = re_sdk::SerializedComponentBatch::new(list_array, component_descr);
        components.insert(batch.descriptor.component, batch);
    }

    Ok(PendingRow {
        row_id,
        timepoint: time_point.clone(),
        components,
    })
}

/// Build a [`Chunk`] given a '**kwargs'-style dictionary of component arrays.
pub fn build_chunk_from_components(
    entity_path: EntityPath,
    timelines: &Bound<'_, PyDict>,
    components_per_descr: &Bound<'_, PyDict>,
) -> PyResult<Chunk> {
    // Create chunk-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    let chunk_id = ChunkId::new();

    // Extract the timeline data
    let (arrays, timeline_names): (Vec<ArrowArrayRef>, Vec<TimelineName>) =
        itertools::process_results(
            timelines.iter().map(|(name, array)| {
                let py_name = name.downcast::<PyString>()?;
                let name: std::borrow::Cow<'_, str> = py_name.extract()?;
                let timeline_name: TimelineName = name.as_ref().into();
                array_to_rust(&array).map(|array| (array, timeline_name))
            }),
            |iter| iter.unzip(),
        )?;

    let timelines: Result<Vec<_>, ChunkError> = arrays
        .into_iter()
        .zip(timeline_names)
        .map(|(array, timeline_name)| {
            let time_type = re_log_types::TimeType::from_arrow_datatype(array.data_type())
                .ok_or_else(|| ChunkError::Malformed {
                    reason: format!("Invalid data_type for timeline: {timeline_name}"),
                })?;
            let timeline = Timeline::new(timeline_name, time_type);
            let timeline_data =
                TimeColumn::read_array(&ArrowArrayRef::from(array)).map_err(|err| {
                    ChunkError::Malformed {
                        reason: format!("Invalid timeline {timeline_name}: {err}"),
                    }
                })?;
            Ok((timeline, timeline_data))
        })
        .collect();

    let timelines: IntMap<TimelineName, TimeColumn> = timelines
        .map_err(|err| PyRuntimeError::new_err(format!("Error converting temporal data: {err}")))?
        .into_iter()
        .map(|(timeline, value)| (*timeline.name(), TimeColumn::new(None, timeline, value)))
        .collect();

    // Extract the component data
    let (arrays, component_descrs): (Vec<ArrowArrayRef>, Vec<ComponentDescriptor>) =
        itertools::process_results(
            components_per_descr.iter().map(|(component_descr, array)| {
                let component_descr = descriptor_to_rust(&component_descr)?;
                array_to_rust(&array).map(|array| (array, component_descr))
            }),
            |iter| iter.unzip(),
        )?;

    let components: Result<Vec<(ComponentDescriptor, _)>, ChunkError> = arrays
        .into_iter()
        .zip(component_descrs)
        .map(|(list_array, descr)| {
            let batch = if let Some(batch) = list_array.downcast_array_ref::<ArrowListArray>() {
                batch.clone()
            } else {
                let offsets =
                    ArrowOffsetBuffer::from_lengths(std::iter::repeat_n(1, list_array.len()));
                let field = ArrowField::new("item", list_array.data_type().clone(), true).into();
                ArrowListArray::try_new(field, offsets, list_array, None).map_err(|err| {
                    ChunkError::Malformed {
                        reason: format!("Failed to wrap in List array: {err}"),
                    }
                })?
            };

            Ok((descr, batch))
        })
        .collect();

    let components = components
        .map_err(|err| PyRuntimeError::new_err(format!("Error converting component data: {err}")))?
        .into_iter()
        .collect();

    let chunk = Chunk::from_auto_row_ids(chunk_id, entity_path, timelines, components)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(chunk)
}

/// Build an Arrow `FixedSizeListArray` of `f32` values directly from a flat numpy array.
///
/// Returns a `NativeArrowArray` handle — the data stays on the Rust side as
/// `Arc<dyn Array>`, completely bypassing PyArrow export/import overhead.
///
/// This bypasses PyArrow's `pa.FixedSizeListArray.from_arrays()` which has ~1.0 us overhead
/// per call regardless of array size. For small fixed-size types like Vec3D (3 floats) and
/// Mat3x3 (9 floats), this overhead dominates.
#[pyfunction]
pub fn build_fixed_size_list_array(
    flat_array: PyReadonlyArray1<'_, f32>,
    list_size: i32,
) -> PyResult<NativeArrowArray> {
    let slice = flat_array
        .as_slice()
        .map_err(|err| PyValueError::new_err(format!("numpy array must be contiguous: {err}")))?;

    let num_elements = slice.len();
    let list_size_usize = list_size as usize;
    if num_elements % list_size_usize != 0 {
        return Err(PyValueError::new_err(format!(
            "flat array length {num_elements} is not a multiple of list_size {list_size}"
        )));
    }

    let values = ArrowFloat32Array::from(slice.to_vec());
    let field = Arc::new(ArrowField::new("item", ArrowDataType::Float32, false));
    let array = ArrowFixedSizeListArray::new(field, list_size, Arc::new(values), None);

    Ok(NativeArrowArray {
        inner: Arc::new(array),
    })
}
