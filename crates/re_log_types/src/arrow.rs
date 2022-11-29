use std::collections::BTreeMap;

use arrow2::{
    array::{Array, PrimitiveArray},
    datatypes::{DataType, Field, TimeUnit},
};

use crate::{TimePoint, TimeType};

pub const OBJPATH_KEY: &str = "RERUN:object_path";
pub const TIMELINE_KEY: &str = "RERUN:timeline";
pub const TIMELINE_SEQUENCE: &str = "Sequence";
pub const TIMELINE_TIME: &str = "Time";

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
