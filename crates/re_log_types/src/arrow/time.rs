use arrow2::{
    array::{
        Int64Array, MapArray, MutableArray, MutableMapArray, MutablePrimitiveArray,
        MutableUtf8Array, StructArray, Utf8Array,
    },
    datatypes::{DataType, Field},
};
use arrow2_convert::{deserialize::ArrowDeserialize, field::ArrowField, serialize::ArrowSerialize};

use crate::{TimeInt, TimePoint, Timeline};

arrow2_convert::arrow_enable_vec_for_type!(TimePoint);

impl ArrowField for TimePoint {
    type Type = Self;

    #[inline]
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

pub struct TimePointIterator<'a>{
  timelines: < &'a<String as arrow2_convert::deserialize::ArrowDeserialize> ::ArrayType as IntoIterator> ::IntoIter,
  times: < &'a<i64 as arrow2_convert::deserialize::ArrowDeserialize> ::ArrayType as IntoIterator> ::IntoIter,
}

impl<'a> Iterator for TimePointIterator<'a> {
    type Item = TimePoint;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let (Some(timeline), Some(time)) =
            (self.timelines.next().flatten(), self.times.next().flatten())
        {
            Some(TimePoint(
                [(
                    Timeline::new(timeline, crate::TimeType::Time),
                    TimeInt::from(*time),
                )]
                .into(),
            ))
        } else {
            None
        }
    }
}

pub struct TimePointArray;

impl<'a> IntoIterator for &'a TimePointArray {
    type Item = TimePoint;
    type IntoIter = TimePointIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        unreachable!("Use iter_from_array_ref");
    }
}

impl arrow2_convert::deserialize::ArrowArray for TimePointArray {
    type BaseArrayType = arrow2::array::MapArray;

    #[inline]
    fn iter_from_array_ref(b: &dyn arrow2::array::Array) -> <&Self as IntoIterator>::IntoIter {
        let arr = b.as_any().downcast_ref::<MapArray>().unwrap();
        let field = arr.field().as_any().downcast_ref::<StructArray>().unwrap();
        let values = field.values();
        assert_eq!(field.validity(), None, "TimePoints should be non-null");

        TimePointIterator {
            timelines: Utf8Array::<i32>::iter_from_array_ref(&*values[0]),
            times: Int64Array::iter_from_array_ref(&*values[1]),
        }
    }
}

impl ArrowDeserialize for TimePoint {
    type ArrayType = TimePointArray;

    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        Some(v)
    }
}

#[test]
fn test_timepoint_roundtrip() {
    use crate::{Time, TimeType, Timeline};
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let mut time_point = TimePoint::default();
    time_point.0.insert(
        Timeline::new("log_time", TimeType::Time),
        Time::from_ns_since_epoch(100).into(),
    );
    let array: Box<dyn Array> = vec![time_point].try_into_arrow().unwrap();

    let time_points: Vec<TimePoint> = TryIntoCollection::try_into_collection(array).unwrap();

    assert_eq!(
        time_points,
        vec![TimePoint(
            [(
                Timeline {
                    name: "log_time".into(),
                    typ: TimeType::Time
                },
                TimeInt(100)
            )]
            .into()
        )]
    );
}
