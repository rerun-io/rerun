//! Methods for handling Arrow datamodel log ingest

use arrow2::{array::Array, chunk::Chunk, datatypes::Field, ffi};
use pyo3::{
    exceptions::{PyAttributeError, PyValueError},
    ffi::Py_uintptr_t,
    types::PyDict,
    types::{IntoPyDict, PyString},
    PyAny, PyResult,
};
use re_log_types::{
    field_types,
    msg_bundle::{self, ComponentBundle, MsgBundle, MsgBundleError},
    LogMsg, MsgId, ObjPath, TimePoint,
};

/// Perform conversion between a pyarrow array to arrow2 types.
fn array_to_rust(
    arrow_array: &PyAny,
    field_name: Option<&str>,
) -> PyResult<(Box<dyn Array>, Field)> {
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
            .map_err(|e| PyValueError::new_err(format!("Error importing Field: {e}")))?;
        let array = ffi::import_array_from_c(*array, field.data_type.clone())
            .map_err(|e| PyValueError::new_err(format!("Error importing Array: {e}")))?;

        if let Some(field_name) = field_name {
            field.name = field_name.to_owned();
        }

        Ok((array, field))
    }
}

#[pyo3::pyfunction]
pub fn get_registered_fields(py: pyo3::Python<'_>) -> PyResult<&PyDict> {
    let pyarrow = py.import("pyarrow")?;
    let pyarrow_field_cls = pyarrow
        .dict()
        .get_item("Field")
        .ok_or_else(|| PyAttributeError::new_err("Module 'pyarrow' has no attribute 'Field'"))?;

    let fields = field_types::iter_registered_field_types()
        .map(|field| {
            let schema = Box::new(ffi::export_field_to_c(field));
            let schema_ptr = &*schema as *const ffi::ArrowSchema;
            pyarrow_field_cls
                .call_method1("_import_from_c", (schema_ptr as Py_uintptr_t,))
                .map(|f| (field.name.clone(), f))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(fields.into_py_dict(py))
}

/// Build an Arrow [`Chunk`] and vector of [`Field`] given a '**kwargs'-style dictionary of
/// component arrays.
pub fn build_chunk_from_components(
    obj_path: &ObjPath,
    components: &PyDict,
    time_point: &TimePoint,
) -> PyResult<LogMsg> {
    let (arrays, fields): (Vec<Box<dyn Array>>, Vec<Field>) = itertools::process_results(
        components.iter().map(|(name, array)| {
            let name = name.downcast::<PyString>()?.to_str()?;
            array_to_rust(array, Some(name))
        }),
        |iter| iter.unzip(),
    )?;

    // Turn the arrays into a `Chunk`
    let chunk = Chunk::try_new(arrays).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let names = fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
    re_log::info!(
        "Logged an arrow msg to path '{}':\n{}",
        obj_path,
        arrow2::io::print::write(&[chunk.clone()], names.as_slice())
    );

    let cmp_bundles = chunk
        .into_arrays()
        .into_iter()
        .zip(names.into_iter())
        .map(|(value, name)| ComponentBundle {
            name,
            value: msg_bundle::wrap_in_listarray(value).boxed(),
        })
        .collect();

    let msg_bundle = MsgBundle::new(
        MsgId::random(),
        obj_path.clone(),
        time_point.clone(),
        cmp_bundles,
    );

    let msg = msg_bundle
        .try_into()
        .map_err(|e: MsgBundleError| PyValueError::new_err(e.to_string()))?;

    Ok(LogMsg::ArrowMsg(msg))
}
