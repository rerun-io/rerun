//! Methods for handling Arrow datamodel log ingest

use std::borrow::Cow;

use arrow::{
    array::{
        make_array, ArrayData as ArrowArrayData, ArrayRef as ArrowArrayRef,
        ListArray as ArrowListArray,
    },
    buffer::OffsetBuffer as ArrowOffsetBuffer,
    datatypes::Field as ArrowField,
    pyarrow::PyArrowType,
};
use pyo3::{
    exceptions::PyRuntimeError,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyString},
    Bound, PyAny, PyResult,
};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, ChunkError, ChunkId, PendingRow, RowId, TimeColumn, TimelineName};
use re_log_types::TimePoint;
use re_sdk::{external::nohash_hasher::IntMap, ComponentDescriptor, EntityPath, Timeline};

/// Perform Python-to-Rust conversion for a `ComponentDescriptor`.
pub fn descriptor_to_rust(component_descr: &Bound<'_, PyAny>) -> PyResult<ComponentDescriptor> {
    let py = component_descr.py();

    let archetype_name = component_descr.getattr(pyo3::intern!(py, "archetype_name"))?;
    let archetype_name: Option<Cow<'_, str>> = if !archetype_name.is_none() {
        Some(archetype_name.extract()?)
    } else {
        None
    };

    let archetype_field_name =
        component_descr.getattr(pyo3::intern!(py, "archetype_field_name"))?;
    let archetype_field_name: Option<Cow<'_, str>> = if !archetype_field_name.is_none() {
        Some(archetype_field_name.extract()?)
    } else {
        None
    };

    let component_name = component_descr.getattr(pyo3::intern!(py, "component_name"))?;
    let component_name: Cow<'_, str> = component_name.extract()?;

    let descr = ComponentDescriptor {
        archetype_name: archetype_name.map(|s| s.as_ref().into()),
        archetype_field_name: archetype_field_name.map(|s| s.as_ref().into()),
        component_name: component_name.as_ref().into(),
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
        components.insert(component_descr, list_array);
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

    let timelines: IntMap<Timeline, TimeColumn> = timelines
        .map_err(|err| PyRuntimeError::new_err(format!("Error converting temporal data: {err}")))?
        .into_iter()
        .map(|(timeline, value)| (timeline, TimeColumn::new(None, timeline, value)))
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
                    ArrowOffsetBuffer::from_lengths(std::iter::repeat(1).take(list_array.len()));
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
