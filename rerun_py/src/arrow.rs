//! Methods for handling Arrow datamodel log ingest

use arrow2::{array::Array, datatypes::Field, ffi};
use itertools::Itertools as _;
use pyo3::{
    exceptions::PyValueError, ffi::Py_uintptr_t, types::PyDict, types::PyString, PyAny, PyResult,
};
use re_log_types::{DataCell, DataRow, EntityPath, RowId, TimePoint};

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
    // Following pattern from: https://github.com/pola-rs/polars/blob/master/examples/python_rust_compiled_function/src/ffi.rs
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

/// Build a [`DataRow`] given a '**kwargs'-style dictionary of component arrays.
pub fn build_data_row_from_components(
    entity_path: &EntityPath,
    components: &PyDict,
    time_point: &TimePoint,
) -> PyResult<DataRow> {
    let row_id = RowId::new(); // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.

    let (arrays, fields): (Vec<Box<dyn Array>>, Vec<Field>) = itertools::process_results(
        components.iter().map(|(name, array)| {
            let name = name.downcast::<PyString>()?.to_str()?;
            array_to_rust(array, Some(name))
        }),
        |iter| iter.unzip(),
    )?;

    let cells = arrays
        .into_iter()
        .zip(fields)
        .map(|(value, field)| DataCell::from_arrow(field.name.into(), value))
        .collect_vec();

    let num_instances = cells.first().map_or(0, |cell| cell.num_instances());
    let row = DataRow::from_cells(
        row_id,
        time_point.clone(),
        entity_path.clone(),
        num_instances,
        cells,
    )
    .map_err(|err| PyValueError::new_err(err.to_string()))?;

    Ok(row)
}
