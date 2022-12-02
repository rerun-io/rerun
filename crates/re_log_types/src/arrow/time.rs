use arrow2::{
    array::{MutableArray, MutableMapArray, MutablePrimitiveArray, MutableUtf8Array},
    datatypes::{DataType, Field},
};
use arrow2_convert::{field::ArrowField, serialize::ArrowSerialize};

use crate::TimePoint;

arrow2_convert::arrow_enable_vec_for_type!(TimePoint);

impl ArrowField for TimePoint {
    type Type = Self;
    fn data_type() -> DataType {
        DataType::Map(
            Box::new(Field::new(
                "entries",
                DataType::Struct(vec![
                    Field::new("timeline", DataType::Utf8, false),
                    Field::new("time", DataType::Int64, false),
                ]),
                true,
            )),
            false,
        )
    }
}

impl ArrowSerialize for TimePoint {
    type MutableArrayType = MutableMapArray;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        let timeline_array: Box<dyn MutableArray> = Box::new(MutableUtf8Array::<i32>::new());
        let time_array: Box<dyn MutableArray> = Box::new(MutablePrimitiveArray::<i64>::new());
        MutableMapArray::try_new(
            <TimePoint as ArrowField>::data_type(),
            vec![timeline_array, time_array],
        )
        .unwrap()
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        let (keys, values): (&mut MutableUtf8Array<i32>, &mut MutablePrimitiveArray<i64>) =
            array.keys_values().ok_or_else(|| {
                arrow2::error::Error::InvalidArgumentError("Error extracting map fields".to_owned())
            })?;

        for (timeline, time) in &v.0 {
            keys.push(Some(timeline.name()));
            values.push(Some(time.as_i64()));
        }
        array.try_push_valid()
    }
}

#[test]
fn test_timepoint_arrow() {
    use crate::{Time, TimeType, Timeline};
    use arrow2::array::Array;
    use arrow2_convert::serialize::TryIntoArrow;

    let mut time_point = TimePoint::default();
    time_point.0.insert(
        Timeline::new("log_time", TimeType::Time),
        Time::now().into(),
    );
    let array: Box<dyn Array> = vec![time_point].try_into_arrow().unwrap();
    dbg!(array);
}
