//! Methods for handling Arrow datamodel log ingest

use std::borrow::Cow;
use std::sync::Arc;

use arrow::array::{
    ArrayData as ArrowArrayData, ArrayRef as ArrowArrayRef, BooleanArray, FixedSizeListArray,
    Float32Array, Float64Array, Int64Array, ListArray as ArrowListArray, StringArray, UInt8Array,
    UInt16Array, UInt32Array, UInt64Array, make_array,
};
use arrow::buffer::{Buffer as ArrowBuffer, OffsetBuffer as ArrowOffsetBuffer, ScalarBuffer};
use arrow::datatypes::Field as ArrowField;
use arrow::pyarrow::PyArrowType;
use numpy::PyReadonlyArray1;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::types::{PyAnyMethods as _, PyDict, PyDictMethods as _, PyString, PyTuple};
use pyo3::{Bound, PyAny, PyResult};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, ChunkError, ChunkId, PendingRow, RowId, TimeColumn, TimelineName};
use re_log_types::TimePoint;
use re_sdk::external::nohash_hasher::IntMap;
use re_sdk::{ComponentDescriptor, EntityPath, Timeline};

/// Perform Python-to-Rust conversion for a `ComponentDescriptor`.
pub fn descriptor_to_rust(component_descr: &Bound<'_, PyAny>) -> PyResult<ComponentDescriptor> {
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

/// Perform conversion between a pyarrow array to arrow types.
///
/// `name` is the name of the Rerun component, and the name of the pyarrow `Field` (column name).
pub fn array_to_rust(arrow_array: &Bound<'_, PyAny>) -> PyResult<ArrowArrayRef> {
    let py_array: PyArrowType<ArrowArrayData> = arrow_array.extract()?;
    Ok(make_array(py_array.0))
}

/// Build an Arrow `FixedSizeListArray` directly from a numpy buffer.
///
/// This avoids the ~1us overhead of going through PyArrow for each component.
/// Supports f32, f64, u32, and u8 element types (covering all FixedSizeList datatypes).
fn numpy_to_fixed_size_list(
    np_array: &Bound<'_, PyAny>,
    list_size: i32,
) -> PyResult<ArrowArrayRef> {
    // Try f32 first (most common: Vec2D/3D/4D, Quaternion, Mat3x3/4x4, Plane3D)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, f32>>() {
        let slice = arr.as_slice()?;
        let values = Float32Array::new(ScalarBuffer::<f32>::from(slice.to_vec()), None);
        let field = Arc::new(ArrowField::new(
            "item",
            arrow::datatypes::DataType::Float32,
            false,
        ));
        return Ok(Arc::new(FixedSizeListArray::new(
            field,
            list_size,
            Arc::new(values),
            None,
        )));
    }
    // Then f64 (DVec2D, Range1D)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, f64>>() {
        let slice = arr.as_slice()?;
        let values = Float64Array::new(ScalarBuffer::<f64>::from(slice.to_vec()), None);
        let field = Arc::new(ArrowField::new(
            "item",
            arrow::datatypes::DataType::Float64,
            false,
        ));
        return Ok(Arc::new(FixedSizeListArray::new(
            field,
            list_size,
            Arc::new(values),
            None,
        )));
    }
    // Then u32 (UVec2D, UVec3D)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u32>>() {
        let slice = arr.as_slice()?;
        let values = UInt32Array::new(ScalarBuffer::<u32>::from(slice.to_vec()), None);
        let field = Arc::new(ArrowField::new(
            "item",
            arrow::datatypes::DataType::UInt32,
            false,
        ));
        return Ok(Arc::new(FixedSizeListArray::new(
            field,
            list_size,
            Arc::new(values),
            None,
        )));
    }
    // Then u8 (UUID, ViewCoordinates)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u8>>() {
        let slice = arr.as_slice()?;
        let values = UInt8Array::new(ScalarBuffer::<u8>::from(slice.to_vec()), None);
        let field = Arc::new(ArrowField::new(
            "item",
            arrow::datatypes::DataType::UInt8,
            false,
        ));
        return Ok(Arc::new(FixedSizeListArray::new(
            field,
            list_size,
            Arc::new(values),
            None,
        )));
    }

    Err(PyTypeError::new_err(
        "numpy array must be float32, float64, uint32, or uint8",
    ))
}

/// Build a plain Arrow primitive array directly from a numpy buffer.
///
/// This avoids the ~1us overhead of going through PyArrow for each component.
/// Supports f32, f64, u16, u32, u64, i64, u8, and bool element types.
fn numpy_to_primitive_array(np_array: &Bound<'_, PyAny>) -> PyResult<ArrowArrayRef> {
    // f32 (Float32, Angle, Radius, Opacity, etc.)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, f32>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(Float32Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // f64 (Float64, Scalar)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, f64>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(Float64Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // u32 (Rgba32/Color)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u32>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(UInt32Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // u16 (ClassId, KeypointId)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u16>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(UInt16Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // u64 (Count)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u64>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(UInt64Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // i64 (Timestamp, VideoTimestamp)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, i64>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(Int64Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // u8 (enums, PixelFormat, ChannelDatatype)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, u8>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(UInt8Array::new(
            ScalarBuffer::from(slice.to_vec()),
            None,
        )));
    }
    // bool (ShowLabels, Visible, etc.)
    if let Ok(arr) = np_array.extract::<PyReadonlyArray1<'_, bool>>() {
        let slice = arr.as_slice()?;
        return Ok(Arc::new(BooleanArray::from(slice.to_vec())));
    }

    Err(PyTypeError::new_err(
        "unsupported numpy dtype for primitive array",
    ))
}

/// Build an Arrow `StringArray` from UTF-8 bytes + offsets numpy arrays.
fn numpy_to_string_array(
    data: &Bound<'_, PyAny>,
    offsets: &Bound<'_, PyAny>,
) -> PyResult<ArrowArrayRef> {
    let data_arr = data.extract::<PyReadonlyArray1<'_, u8>>()?;
    let data_slice = data_arr.as_slice()?;
    let offsets_arr = offsets.extract::<PyReadonlyArray1<'_, i32>>()?;
    let offsets_slice = offsets_arr.as_slice()?;

    let arrow_offsets =
        ArrowOffsetBuffer::new(ScalarBuffer::<i32>::from(offsets_slice.to_vec()));
    let values = ArrowBuffer::from(data_slice.to_vec());

    let string_array = StringArray::new(arrow_offsets, values, None);
    Ok(Arc::new(string_array))
}

/// Build an Arrow `ListArray` from flat data + offsets numpy arrays.
///
/// `inner_size > 0`: inner elements are `FixedSizeList<T, inner_size>`.
/// `inner_size == 0`: inner elements are plain primitives.
fn numpy_to_list_array(
    data: &Bound<'_, PyAny>,
    offsets: &Bound<'_, PyAny>,
    inner_size: i32,
) -> PyResult<ArrowArrayRef> {
    let offsets_arr = offsets.extract::<PyReadonlyArray1<'_, i32>>()?;
    let offsets_slice = offsets_arr.as_slice()?;
    let arrow_offsets =
        ArrowOffsetBuffer::new(ScalarBuffer::<i32>::from(offsets_slice.to_vec()));

    let inner_values: ArrowArrayRef = if inner_size > 0 {
        numpy_to_fixed_size_list(data, inner_size)?
    } else {
        numpy_to_primitive_array(data)?
    };

    let field = Arc::new(ArrowField::new(
        "item",
        inner_values.data_type().clone(),
        true,
    ));
    let list_array = ArrowListArray::try_new(field, arrow_offsets, inner_values, None)
        .map_err(|err| PyTypeError::new_err(format!("Failed to build ListArray: {err}")))?;
    Ok(Arc::new(list_array))
}

/// Extract an Arrow array from a Python value.
///
/// Supported formats:
/// - `(numpy_array, list_size)` 2-tuple: FixedSizeList or primitive
/// - `(data_numpy, offsets_numpy, inner_size)` 3-tuple: variable-length list or string
/// - PyArrow array (fallback)
///
/// Convention for 2-tuples: `list_size == 0` → plain primitive.
/// Convention for 3-tuples: `inner_size == -1` → UTF-8 string,
///   `inner_size == 0` → List\<primitive\>, `inner_size > 0` → List\<FixedSizeList\>.
fn value_to_arrow_array(value: &Bound<'_, PyAny>) -> PyResult<ArrowArrayRef> {
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        match tuple.len()? {
            2 => {
                let np_array = tuple.get_item(0)?;
                let list_size: i32 = tuple.get_item(1)?.extract()?;
                if list_size == 0 {
                    return numpy_to_primitive_array(&np_array);
                }
                return numpy_to_fixed_size_list(&np_array, list_size);
            }
            3 => {
                let data = tuple.get_item(0)?;
                let offsets = tuple.get_item(1)?;
                let inner_size: i32 = tuple.get_item(2)?.extract()?;
                if inner_size == -1 {
                    return numpy_to_string_array(&data, &offsets);
                }
                return numpy_to_list_array(&data, &offsets, inner_size);
            }
            _ => {}
        }
    }
    array_to_rust(value)
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
    for (component_descr, value) in components_per_descr {
        let component_descr = descriptor_to_rust(&component_descr)?;
        let list_array = value_to_arrow_array(&value)?;
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
            components_per_descr.iter().map(|(component_descr, value)| {
                let component_descr = descriptor_to_rust(&component_descr)?;
                value_to_arrow_array(&value).map(|array| (array, component_descr))
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
