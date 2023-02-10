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

        ListArray::<i32>::default_datatype(struct_type)
        //TODO(john) Wrapping the DataType in Extension exposes a bug in arrow2::io::ipc
        //DataType::Extension("TimePoint".to_owned(), Box::new(list_type), None)
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
        let DataType::List(inner) = data_type.to_logical_type() else { unreachable!() };
        let str_array = MutableStructArray::new(
            inner.data_type.clone(),
            vec![timeline_array, time_type_array, time_array],
        );
        MutableListArray::new_from(str_array, data_type, 0)
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

// ----------------------------------------------------------------------------

pub struct TimePointIterator<'a> {
    time_points: <&'a ListArray<i32> as IntoIterator>::IntoIter,
}

impl<'a> Iterator for TimePointIterator<'a> {
    type Item = TimePoint;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.time_points.next().flatten().map(|time_point| {
            let struct_arr = time_point
                .as_any()
                .downcast_ref::<StructArray>()
                .expect("StructArray");
            let values = struct_arr.values();
            let timelines = values[0]
                .as_any()
                .downcast_ref::<Utf8Array<i32>>()
                .expect("timelines");
            let types = values[1]
                .as_any()
                .downcast_ref::<UInt8Array>()
                .expect("types");
            let times = values[2]
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("times");

            let time_points = timelines.iter().zip(types.iter()).zip(times.iter()).map(
                |((timeline, ty), time)| {
                    (
                        Timeline::new(
                            timeline.unwrap(),
                            num_traits::FromPrimitive::from_u8(*ty.unwrap())
                                .expect("valid TimeType"),
                        ),
                        TimeInt::from(*time.unwrap()),
                    )
                },
            );

            time_points.collect()
        })
    }
}

// ----------------------------------------------------------------------------
pub struct TimePointArray;

impl<'a> IntoIterator for &'a TimePointArray {
    type Item = TimePoint;

    type IntoIter = TimePointIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        panic!("Use iter_from_array_ref. This is a quirk of the way the traits work in arrow2_convert.");
    }
}

impl arrow2_convert::deserialize::ArrowArray for TimePointArray {
    type BaseArrayType = arrow2::array::MapArray;

    #[inline]
    fn iter_from_array_ref(b: &dyn arrow2::array::Array) -> <&Self as IntoIterator>::IntoIter {
        let arr = b.as_any().downcast_ref::<ListArray<i32>>().unwrap();
        assert_eq!(arr.validity(), None, "TimePoints should be non-null");

        TimePointIterator {
            time_points: arr.into_iter(),
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

// ----------------------------------------------------------------------------

#[test]
fn test_timepoint_roundtrip() {
    use crate::datagen;
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let time_points_in = vec![
        TimePoint::from([
            datagen::build_log_time(crate::Time::from_ns_since_epoch(100)),
            datagen::build_frame_nr(1234.into()),
        ]),
        TimePoint::from([
            datagen::build_log_time(crate::Time::from_ns_since_epoch(200)),
            datagen::build_frame_nr(2345.into()),
        ]),
    ];

    let array: Box<dyn Array> = time_points_in.try_into_arrow().unwrap();
    let time_points_out: Vec<TimePoint> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(time_points_in, time_points_out);
}
