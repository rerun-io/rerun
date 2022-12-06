use arrow2::{
    array::{
        Int64Array, ListArray, MutableArray, MutableListArray, MutablePrimitiveArray,
        MutableStructArray, MutableUtf8Array, StructArray, UInt8Array, Utf8Array,
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
        //TODO(john) Use Dictionary type
        //let time_type_values = Utf8Array::<i32>::from_slice(["Time", "Sequence"]);
        //let time_type = DataType::Dictionary(
        //    i32::KEY_TYPE,
        //    Box::new(time_type_values.data_type().clone()),
        //    false,
        //);
        let time_type = DataType::UInt8;

        let struct_type = DataType::Struct(vec![
            Field::new("timeline", DataType::Utf8, false),
            Field::new("type", time_type, false),
            Field::new("time", DataType::Int64, false),
        ]);

        let list_type = ListArray::<i32>::default_datatype(struct_type);

        DataType::Extension("TimePoint".to_owned(), Box::new(list_type), None)
    }
}

impl ArrowSerialize for TimePoint {
    type MutableArrayType = MutableListArray<i32, MutableStructArray>;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        let timeline_array: Box<dyn MutableArray> = Box::new(MutableUtf8Array::<i32>::new());
        let time_type_array: Box<dyn MutableArray> = Box::new(MutablePrimitiveArray::<u8>::new());
        let time_array: Box<dyn MutableArray> = Box::new(MutablePrimitiveArray::<i64>::new());

        let data_type = Self::data_type();
        if let DataType::List(inner) = data_type.to_logical_type() {
            let str_array = MutableStructArray::try_new(
                inner.data_type.clone(),
                vec![timeline_array, time_type_array, time_array],
                None,
            )
            .unwrap();
            MutableListArray::new_from(str_array, data_type, 0)
        } else {
            panic!("Outer datatype should be List");
        }
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        let struct_array = array.mut_values();
        for (timeline, time) in &v.0 {
            <String as ArrowSerialize>::arrow_serialize(
                &timeline.name().to_string(),
                struct_array.value(0).unwrap(),
            )?;
            <u8 as ArrowSerialize>::arrow_serialize(
                &(timeline.typ() as u8),
                struct_array.value(1).unwrap(),
            )?;
            <i64 as ArrowSerialize>::arrow_serialize(
                &time.as_i64(),
                struct_array.value(2).unwrap(),
            )?;
            struct_array.push(true);
        }
        array.try_push_valid()
    }
}

pub struct TimePointIterator<'a> {
    timelines: <&'a <String as ArrowDeserialize>::ArrayType as IntoIterator>::IntoIter,
    types: <&'a <u8 as ArrowDeserialize>::ArrayType as IntoIterator>::IntoIter,
    times: <&'a <i64 as ArrowDeserialize>::ArrayType as IntoIterator>::IntoIter,
}

impl<'a> Iterator for TimePointIterator<'a> {
    type Item = TimePoint;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let (Some(timeline), Some(ty), Some(time)) = (
            self.timelines.next().flatten(),
            self.types.next().flatten(),
            self.times.next().flatten(),
        ) {
            Some(TimePoint(
                [(
                    Timeline::new(
                        timeline,
                        num_traits::FromPrimitive::from_u8(*ty).expect("valid TimeType"),
                    ),
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
        let arr = b.as_any().downcast_ref::<ListArray<i32>>().unwrap();
        todo!();

        //let values = arr.values();
        //assert_eq!(arr.validity(), None, "TimePoints should be non-null");

        //TimePointIterator {
        //    timelines: Utf8Array::<i32>::iter_from_array_ref(&*values[0]),
        //    types: UInt8Array::iter_from_array_ref(&*values[1]),
        //    times: Int64Array::iter_from_array_ref(&*values[2]),
        //}
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
    use crate::{TimeType, Timeline};
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let time_points_in = vec![
        TimePoint(
            [
                (Timeline::new("log_time", TimeType::Time), TimeInt(100)),
                (Timeline::new("seq1", TimeType::Sequence), 1234.into()),
            ]
            .into(),
        ),
        TimePoint(
            [
                (Timeline::new("log_time", TimeType::Time), TimeInt(200)),
                (Timeline::new("seq1", TimeType::Sequence), 2345.into()),
            ]
            .into(),
        ),
    ];

    let array: Box<dyn Array> = time_points_in.try_into_arrow().unwrap();
    dbg!(&array);
    //let time_points_out: Vec<TimePoint> = TryIntoCollection::try_into_collection(array).unwrap();
    //assert_eq!(time_points_in, time_points_out);
}
