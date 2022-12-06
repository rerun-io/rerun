use std::collections::BTreeMap;

use arrow2::{
    array::{Array, ListArray, StructArray},
    buffer::Buffer,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};

use re_log_types::{datagen, ArrowMsg, LogMsg, MsgId, ObjPath, TimePoint, ENTITY_PATH_KEY};

/// Create a [`StructArray`] from an array of (name, Array) tuples
pub fn components_as_struct_array(components: &[(&str, Box<dyn Array>)]) -> StructArray {
    let data_types = DataType::Struct(
        components
            .iter()
            .map(|(name, data)| Field::new(*name, data.data_type().clone(), true))
            .collect(),
    );

    let data_arrays = components.iter().map(|(_, data)| data.clone()).collect();

    StructArray::new(data_types, data_arrays, None)
}

/// Build the [`LogMsg`] from the [`StructArray`]
pub fn build_arrow_log_msg(
    obj_path: &ObjPath,
    array: &StructArray,
    time_point: &TimePoint,
) -> Result<LogMsg, arrow2::error::Error> {
    re_log::info!(
        "Logged an arrow msg to path '{}'  with components {:?}",
        obj_path,
        array
            .fields()
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
    );

    let components = array
        .fields()
        .iter()
        .zip(array.values().iter())
        .map(|(field, array)| ("", Schema::from(vec![field.clone()]), array.clone()));

    let (schema, chunk) = datagen::build_message(obj_path, time_point, components);

    // Build columns for timeline data
    //let (mut fields, mut cols): (Vec<Field>, Vec<Box<dyn Array>>) = build_time_cols(time_point).unzip();
    //fields.push(arrow2::datatypes::Field::new( "components", data_col.data_type().clone(), true,));
    //cols.push(data_col.boxed());

    Ok(LogMsg::ArrowMsg(ArrowMsg {
        msg_id: MsgId::random(),
        schema,
        chunk,
    }))
}
