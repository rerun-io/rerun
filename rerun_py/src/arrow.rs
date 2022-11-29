//! Methods for handling Arrow datamodel log ingest

use std::collections::BTreeMap;

use arrow2::{
    array::{Array, ListArray, StructArray},
    buffer::Buffer,
    chunk::Chunk,
    datatypes::{Field, Schema},
    ffi,
    io::ipc::write::StreamWriter,
};
use pyo3::{exceptions::PyTypeError, ffi::Py_uintptr_t, PyAny, PyResult};
use re_log_types::{
    arrow::{build_time_cols, ENTITY_PATH_KEY},
    ArrowMsg, LogMsg, MsgId, ObjPath, TimePoint,
};

/// Perform conversion between a pyarrow array to arrow2 types.
/// This operation does not copy data.
pub fn array_to_rust(arrow_array: &PyAny) -> PyResult<(Box<dyn Array>, Field)> {
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

/// Create a `LogMsg` out of Arrow Schema and Chunk
pub fn serialize_arrow_msg(
    schema: &Schema,
    chunk: &Chunk<Box<dyn Array>>,
) -> Result<LogMsg, arrow2::error::Error> {
    // TODO(jleibs):
    // This stream-writer interface re-encodes and transmits the schema on every send
    // I believe We can optimize this using some combination of calls to:
    // https://docs.rs/arrow2/latest/arrow2/io/ipc/write/fn.write.html

    let mut data = Vec::<u8>::new();
    let mut writer = StreamWriter::new(&mut data, Default::default());
    writer.start(schema, None)?;
    writer.write(chunk, None)?;
    writer.finish()?;

    //let data_path = DataPath::new(obj_path, FieldName::from(field_name));

    let msg = ArrowMsg {
        msg_id: MsgId::random(),
        //data_path,
        data,
    };

    Ok(LogMsg::ArrowMsg(msg))
}

pub fn build_arrow_log_msg(
    obj_path: &ObjPath,
    array: &PyAny,
    time_point: &TimePoint,
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

    let data_col = ListArray::<i32>::try_new(
        ListArray::<i32>::default_datatype(array.data_type().clone()), // data_type
        Buffer::from(vec![0, array.values()[0].len() as i32]),         // offsets
        array.clone().boxed(),                                         // values
        None,                                                          // validity
    )
    .map_err(|err| PyTypeError::new_err(err.to_string()))?;

    // Build columns for timeline data
    let (mut fields, mut cols): (Vec<Field>, Vec<Box<dyn Array>>) =
        build_time_cols(time_point).unzip();

    fields.push(arrow2::datatypes::Field::new(
        "components",
        data_col.data_type().clone(),
        true,
    ));
    cols.push(data_col.boxed());

    let metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), obj_path.to_string())]);
    dbg!(&metadata);
    let schema = Schema { fields, metadata };
    let chunk = Chunk::new(cols);

    dbg!(&schema);
    dbg!(&chunk);

    serialize_arrow_msg(&schema, &chunk).map_err(|err| PyTypeError::new_err(err.to_string()))
}
