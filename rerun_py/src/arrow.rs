//! Methods for handling Arrow datamodel log ingest

use arrow2::{array::Array, datatypes::Field, ffi};
use pyo3::{
    exceptions::{PyAttributeError, PyValueError},
    ffi::Py_uintptr_t,
    types::PyDict,
    types::{IntoPyDict, PyString},
    PyAny, PyResult,
};
use re_log_types::{
    component_types,
    msg_bundle::{self, ComponentBundle, MsgBundle, MsgBundleError},
    EntityPath, LogMsg, MsgId, TimePoint,
};

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
            .map_err(|e| PyValueError::new_err(format!("Error importing Field: {e}")))?;

        // There is a bad incompatibility between pyarrow and arrow2-convert
        // Force the type to be correct.
        // https://github.com/rerun-io/rerun/issues/795s
        if let Some(name) = name {
            if name == <component_types::Tensor as msg_bundle::Component>::name() {
                field.data_type = <component_types::Tensor as re_log_types::external::arrow2_convert::field::ArrowField>::data_type();
            } else if name == <component_types::Rect2D as msg_bundle::Component>::name() {
                field.data_type = <component_types::Rect2D as re_log_types::external::arrow2_convert::field::ArrowField>::data_type();
            }
        }

        let array = ffi::import_array_from_c(*array, field.data_type.clone())
            .map_err(|e| PyValueError::new_err(format!("Error importing Array: {e}")))?;

        if let Some(name) = name {
            field.name = name.to_owned();
        }

        Ok((array, field))
    }
}

#[pyo3::pyfunction]
pub fn get_registered_component_names(py: pyo3::Python<'_>) -> PyResult<&PyDict> {
    let pyarrow = py.import("pyarrow")?;
    let pyarrow_field_cls = pyarrow
        .dict()
        .get_item("Field")
        .ok_or_else(|| PyAttributeError::new_err("Module 'pyarrow' has no attribute 'Field'"))?;

    let fields = component_types::iter_registered_field_types()
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

/// Build a [`LogMsg`] and vector of [`Field`] given a '**kwargs'-style dictionary of
/// component arrays.
pub fn build_chunk_from_components(
    entity_path: &EntityPath,
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

    let cmp_bundles = arrays
        .into_iter()
        .zip(fields.into_iter())
        .map(|(value, field)| {
            ComponentBundle::new(field.name.into(), msg_bundle::wrap_in_listarray(value))
        })
        .collect();

    let msg_bundle = MsgBundle::new(
        MsgId::random(),
        entity_path.clone(),
        time_point.clone(),
        cmp_bundles,
    );

    let msg = msg_bundle
        .try_into()
        .map_err(|e: MsgBundleError| PyValueError::new_err(e.to_string()))?;

    Ok(LogMsg::ArrowMsg(msg))
}
