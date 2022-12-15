//! Methods for handling Arrow datamodel log ingest

use arrow2::{
    array::{Array, StructArray},
    datatypes::Field,
    ffi,
};
use pyo3::{
    exceptions::{PyAttributeError, PyTypeError},
    ffi::Py_uintptr_t,
    types::PyList,
    PyAny, PyResult,
};
use re_log_types::{field_types, LogMsg, ObjPath, TimePoint};

/// Perform conversion between a pyarrow array to arrow2 types.
fn array_to_rust(arrow_array: &PyAny) -> PyResult<(Box<dyn Array>, Field)> {
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
        let field = ffi::import_field_from_c(schema.as_ref()).unwrap();
        let array = ffi::import_array_from_c(*array, field.data_type.clone()).unwrap();
        Ok((array, field))
    }
}

#[pyo3::pyfunction]
pub fn get_registered_fields(py: pyo3::Python<'_>) -> PyResult<&PyAny> {
    let pyarrow = py.import("pyarrow")?;
    let pyarrow_field = pyarrow
        .dict()
        .get_item("Field")
        .ok_or_else(|| PyAttributeError::new_err("Module 'pyarrow' has no attribute 'Field'"))?;

    let fields = field_types::iter_registered_field_types()
        .map(|field| {
            let schema = Box::new(ffi::export_field_to_c(field));
            let schema_ptr = &*schema as *const ffi::ArrowSchema;
            pyarrow_field.call_method1("_import_from_c", (schema_ptr as Py_uintptr_t,))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PyList::new(py, fields))
}

pub fn build_arrow_log_msg_from_py(
    obj_path: &ObjPath,
    array: &PyAny,
    _time_point: &TimePoint,
) -> PyResult<LogMsg> {
    let (array, _field) = array_to_rust(array)?;

    let array = array
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| PyTypeError::new_err("Array should be a StructArray."))?;

    re_log::info!(
        "Logged an arrow msg to path '{}'  with components {:?}",
        obj_path,
        array
            .fields()
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
    );

    /*
    rerun_sdk::arrow::build_arrow_log_msg(obj_path, time_point, array)
        .map_err(|err| PyTypeError::new_err(err.to_string()))
        */
    PyResult::Err(PyTypeError::new_err(
        "TODO(jleibs): Python Arrow Logging is currently broken!",
    ))
}
