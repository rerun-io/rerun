use arrow2::{
    array::Array,
    chunk::Chunk,
    datatypes::{Field, Schema},
};

//use arrow2_convert::field::ArrowField;
use re_log_types::{
    datagen::wrap_in_listarray,
    msg_bundle::{ComponentBundle, MessageBundle},
    ArrowMsg, ComponentNameRef, LogMsg, MsgId, ObjPath, TimePoint,
};

/// Build the [`LogMsg`] from the [`StructArray`]
pub fn build_arrow_log_msg(
    obj_path: &ObjPath,
    time_point: &TimePoint,
    components: impl IntoIterator<Item = (ComponentNameRef<'static>, Box<dyn Array>)>,
) -> Result<LogMsg, arrow2::error::Error> {
    let message_bundle = MessageBundle {
        obj_path: obj_path.clone(),
        time_point: time_point.clone(),
        components: components
            .into_iter()
            .map(|(name, arr)| {
                let wrapped = wrap_in_listarray(arr).boxed();
                ComponentBundle {
                    name,
                    field: Field::new(name, wrapped.data_type().clone(), false),
                    component: wrapped,
                }
            })
            .collect(),
    };

    let (schema, chunk) =
        TryInto::<(Schema, Chunk<Box<dyn Array>>)>::try_into(message_bundle).unwrap();

    Ok(LogMsg::ArrowMsg(ArrowMsg {
        msg_id: MsgId::random(),
        schema,
        chunk,
    }))
}
