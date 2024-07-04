//! Methods for handling Arrow datamodel log ingest

use std::collections::BTreeMap;

use arrow2::{
    array::{Array, ListArray, PrimitiveArray},
    datatypes::Field,
    ffi,
    offset::Offsets,
};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    ffi::Py_uintptr_t,
    types::{PyDict, PyString},
    PyAny, PyResult,
};

use re_chunk::{Chunk, ChunkError, ChunkId, ChunkTimeline, PendingRow, RowId};
use re_log_types::TimePoint;
use re_sdk::{ComponentName, EntityPath, Timeline};

/// Perform conversion between a pyarrow array to arrow2 types.
///
/// `name` is the name of the Rerun component, and the name of the pyarrow `Field` (column name).
fn array_to_rust(arrow_array: &PyAny, name: Option<&str>) -> PyResult<(Box<dyn Array>, Field)> {
    // prepare pointers to receive the Array struct
    let array = Box::new(ffi::ArrowArray::empty());
    let schema = Box::new(ffi::ArrowSchema::empty());

    let array_ptr = &*array as *const ffi::ArrowArray;
    let schema_ptr = &*schema as *const ffi::ArrowSchema;

    // make the conversion through PyArrow's private API
    // this changes the pointer's memory and is thus unsafe. In particular, `_export_to_c` can go out of bounds
    arrow_array.call_method1(
        "_export_to_c",
        (array_ptr as Py_uintptr_t, schema_ptr as Py_uintptr_t),
    )?;

    #[allow(unsafe_code)]
    // SAFETY:
    // TODO(jleibs): Convince ourselves that this is safe
    // Following pattern from: https://github.com/pola-rs/polars/blob/1c6b7b70e935fe70384fc0d1ca8d07763011d8b8/examples/python_rust_compiled_function/src/ffi.rs
    unsafe {
        let mut field = ffi::import_field_from_c(schema.as_ref())
            .map_err(|err| PyValueError::new_err(format!("Error importing Field: {err}")))?;

        let array = ffi::import_array_from_c(*array, field.data_type.clone())
            .map_err(|err| PyValueError::new_err(format!("Error importing Array: {err}")))?;

        if let Some(name) = name {
            field.name = name.to_owned();
        }

        Ok((array, field))
    }
}

/// Build a [`PendingRow`] given a '**kwargs'-style dictionary of component arrays.
pub fn build_row_from_components(
    components: &PyDict,
    time_point: &TimePoint,
) -> PyResult<PendingRow> {
    // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    // TODO(emilk): move to before we arrow-serialize the data
    let row_id = RowId::new();

    let (arrays, fields): (Vec<Box<dyn Array>>, Vec<Field>) = itertools::process_results(
        components.iter().map(|(name, array)| {
            let name = name.downcast::<PyString>()?.to_str()?;
            array_to_rust(array, Some(name))
        }),
        |iter| iter.unzip(),
    )?;

    let components = arrays
        .into_iter()
        .zip(fields)
        .map(|(value, field)| (field.name.into(), value))
        .collect();

    Ok(PendingRow {
        row_id,
        timepoint: time_point.clone(),
        components,
    })
}

/// Build a [`Chunk`] given a '**kwargs'-style dictionary of component arrays.
pub fn build_chunk_from_components(
    entity_path: EntityPath,
    timelines: &PyDict,
    components: &PyDict,
) -> PyResult<Chunk> {
    // Create chunk-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    let chunk_id = ChunkId::new();

    // Extract the timeline data
    let (arrays, fields): (Vec<Box<dyn Array>>, Vec<Field>) = itertools::process_results(
        timelines.iter().map(|(name, array)| {
            let name = name.downcast::<PyString>()?.to_str()?;
            array_to_rust(array, Some(name))
        }),
        |iter| iter.unzip(),
    )?;

    let timelines: Result<Vec<_>, ChunkError> = arrays
        .into_iter()
        .zip(fields)
        .map(|(value, field)| {
            let timeline = match field.data_type() {
                arrow2::datatypes::DataType::Int64 => {
                    Ok(Timeline::new_sequence(field.name.clone()))
                }
                arrow2::datatypes::DataType::Timestamp(_, _) => {
                    Ok(Timeline::new_temporal(field.name.clone()))
                }
                _ => Err(ChunkError::Malformed {
                    reason: format!("Invalid data_type for timeline: {}", field.name),
                }),
            }?;
            let timeline_data = value
                .as_any()
                .downcast_ref::<PrimitiveArray<i64>>()
                .ok_or_else(|| ChunkError::Malformed {
                    reason: format!("Invalid primitive array for timeline: {}", field.name),
                })?
                .clone();
            Ok((timeline, timeline_data))
        })
        .collect();

    let timelines: BTreeMap<Timeline, ChunkTimeline> = timelines
        .map_err(|err| PyRuntimeError::new_err(format!("Error converting temporal data: {err}")))?
        .into_iter()
        .map(|(timeline, value)| (timeline, ChunkTimeline::new(None, timeline, value)))
        .collect();

    // Extract the component data
    let (arrays, fields): (Vec<Box<dyn Array>>, Vec<Field>) = itertools::process_results(
        components.iter().map(|(name, array)| {
            let name = name.downcast::<PyString>()?.to_str()?;
            array_to_rust(array, Some(name))
        }),
        |iter| iter.unzip(),
    )?;

    let components: Result<Vec<_>, ChunkError> = arrays
        .into_iter()
        .zip(fields)
        .map(|(value, field)| {
            let batch = if let Some(batch) = value.as_any().downcast_ref::<ListArray<i32>>() {
                batch.clone()
            } else {
                let offsets = Offsets::try_from_lengths(std::iter::repeat(1).take(value.len()))
                    .map_err(|err| ChunkError::Malformed {
                        reason: format!("Failed to create offsets: {err}"),
                    })?;
                let data_type = ListArray::<i32>::default_datatype(value.data_type().clone());
                ListArray::<i32>::try_new(data_type, offsets.into(), value, None).map_err(
                    |err| ChunkError::Malformed {
                        reason: format!("Failed to wrap in List array: {err}"),
                    },
                )?
            };

            Ok((field.name.into(), batch))
        })
        .collect();

    let components: BTreeMap<ComponentName, ListArray<i32>> = components
        .map_err(|err| PyRuntimeError::new_err(format!("Error converting component data: {err}")))?
        .into_iter()
        .collect();

    let mut all_lengths = timelines
        .values()
        .map(|timeline| (timeline.name(), timeline.num_rows()))
        .chain(
            components
                .iter()
                .map(|(component, array)| (component.as_str(), array.len())),
        );

    if let Some((_, expected)) = all_lengths.next() {
        for (name, len) in all_lengths {
            if len != expected {
                return Err(PyRuntimeError::new_err(format!(
                    "Mismatched lengths: '{name}' has length {len} but expected {expected}",
                )));
            }
        }
    }

    let chunk = Chunk::from_auto_row_ids(chunk_id, entity_path, timelines, components)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(chunk)
}
