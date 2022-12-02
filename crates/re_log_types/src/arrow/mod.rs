pub mod field_types;
mod time;
pub use time::*;

use arrow2::{array::MutableStructArray, datatypes::DataType};

use arrow2_convert::{field::ArrowField, serialize::ArrowSerialize};

pub mod exports {
    pub use arrow2_convert::serialize::TryIntoArrow;
}

use crate::{MsgId, ObjPath, Time, TimePoint, TimeType, Timeline};

pub struct ArrowLogMsg<C>
where
    C: ArrowField,
{
    msg_id: MsgId,
    time_point: TimePoint,
    object_path: ObjPath,
    components: C,
}

impl<C> ArrowField for ArrowLogMsg<C>
where
    C: ArrowField,
{
    type Type = Self;

    fn data_type() -> DataType {
        DataType::Extension(
            "ArrowLogMsg".to_owned(),
            Box::new(DataType::Struct(vec![
                <re_tuid::Tuid as ArrowField>::field("msg_id"),
                <TimePoint as ArrowField>::field("time_point"),
                <String as ArrowField>::field("object_path"),
                //<C as ArrowField>::field("components"),
            ])),
            None,
        )
    }
}

impl<C> ArrowSerialize for ArrowLogMsg<C>
where
    C: ArrowField<Type = C> + ArrowSerialize + 'static,
{
    type MutableArrayType = MutableStructArray;

    fn new_array() -> Self::MutableArrayType {
        let msg_id = Box::new(<re_tuid::Tuid as ArrowSerialize>::new_array());
        let time_point = Box::new(<TimePoint as ArrowSerialize>::new_array());
        let object_path = Box::new(<String as ArrowSerialize>::new_array());
        //let components = Box::new(<C as ArrowSerialize>::new_array());
        MutableStructArray::new(
            Self::data_type(),
            vec![
                msg_id,
                time_point,
                object_path,
                //components
            ],
        )
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        <re_tuid::Tuid as ArrowSerialize>::arrow_serialize(&v.msg_id.0, array.value(0).unwrap())?;
        <TimePoint as ArrowSerialize>::arrow_serialize(&v.time_point, array.value(1).unwrap())?;
        <String as ArrowSerialize>::arrow_serialize(
            &v.object_path.to_string(),
            array.value(2).unwrap(),
        )?;
        //<C as ArrowSerialize>::arrow_serialize(&v.components, array.value(3).unwrap())?;
        Ok(())
    }
}

#[test]
fn test_arrow_log_msg() {
    use arrow2::array::Array;
    use exports::TryIntoArrow;

    let mut time_point = TimePoint::default();
    time_point.0.insert(
        Timeline::new("log_time", TimeType::Time),
        Time::now().into(),
    );

    let msgs = [
        ArrowLogMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            object_path: ObjPath::from("obj1"),
            components: field_types::Point2D { x: 0.0, y: 1.0 },
        },
        ArrowLogMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            object_path: ObjPath::from("obj1"),
            components: field_types::Point2D { x: 1.0, y: 0.0 },
        },
    ];

    let array: Box<dyn Array> = msgs.try_into_arrow().unwrap();
    println!("{:#?}", array);
}

pub mod util {
    use std::collections::BTreeMap;

    use arrow2::{
        array::{Array, ListArray, MutableArray, PrimitiveArray, StructArray},
        buffer::Buffer,
        chunk::Chunk,
        datatypes::{DataType, Field, Schema, TimeUnit},
        error::Error,
    };
    use arrow2_convert::serialize::ArrowSerialize;

    use crate::{MsgId, TimePoint, TimeType};

pub type ComponentName = String;
pub type ComponentNameRef<'a> = &'a str;

pub const ENTITY_PATH_KEY: &str = "RERUN:entity_path";
    pub const TIMELINE_KEY: &str = "RERUN:timeline";
    pub const TIMELINE_SEQUENCE: &str = "Sequence";
    pub const TIMELINE_TIME: &str = "Time";

    pub fn build_log_msg_array(
        msg_id: MsgId,
        time_point: &TimePoint,
        components: StructArray,
    ) -> Result<(Chunk<Box<dyn Array>>, Schema), arrow2::error::Error> {
        let msg_id = {
            let mut m = re_tuid::Tuid::new_array();
            re_tuid::Tuid::arrow_serialize(&msg_id.0, &mut m)?;
            m.as_box()
        };

        let timelines = {
            // Build columns for timeline data
            let (fields, cols): (Vec<Field>, Vec<Box<dyn Array>>) =
                build_time_cols(time_point).unzip();
            StructArray::try_new(DataType::Struct(fields), cols, None)?
        };

        let schema = Schema::from(vec![
            Field::new("msg_id", msg_id.data_type().clone(), false),
            Field::new("timelines", timelines.data_type().clone(), false),
            Field::new("components", components.data_type().clone(), false),
        ]);

        let chunk = Chunk::try_new(vec![msg_id, timelines.boxed(), components.boxed()])?;

        Ok((chunk, schema))
    }

    /// Build a iterator of (field, col) for all timelines in `time_point`
    pub fn build_time_cols(
        time_point: &TimePoint,
    ) -> impl Iterator<Item = (Field, Box<dyn Array>)> + '_ {
        time_point.0.iter().map(|(timeline, time)| {
            let (datatype, meta_value) = match timeline.typ() {
                TimeType::Sequence => (DataType::Int64, TIMELINE_SEQUENCE),
                TimeType::Time => (
                    DataType::Timestamp(TimeUnit::Nanosecond, None),
                    TIMELINE_TIME,
                ),
            };
            let arr = PrimitiveArray::from([Some(time.as_i64())]).to(datatype);
            let field =
                Field::new(timeline.name().as_str(), arr.data_type().clone(), false).with_metadata(
                    BTreeMap::from([(TIMELINE_KEY.to_owned(), meta_value.to_owned())]),
                );
            (field, arr.boxed())
        })
    }

    pub fn filter_time_cols<'a>(
        fields: &'a [Field],
        cols: &'a [Box<dyn Array>],
    ) -> impl Iterator<Item = (&'a Field, &'a Box<dyn Array>)> {
        fields
            .iter()
            .zip(cols.iter())
            .filter(|(field, _)| field.metadata.contains_key(TIMELINE_KEY))
    }

    /// Wrap `field_array` in a single-element `ListArray`
    pub fn wrap_in_listarray(field_array: Box<dyn Array>) -> Result<ListArray<i32>, Error> {
        let datatype = ListArray::<i32>::default_datatype(field_array.data_type().clone());
        let offsets = Buffer::from(vec![0, field_array.len() as i32]);
        let values = field_array;
        let validity = None;
        ListArray::<i32>::try_new(datatype, offsets, values, validity)
    }
}
