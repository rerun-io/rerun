use arrow2::{
    array::{MutableFixedSizeListArray, MutablePrimitiveArray},
    datatypes::DataType,
};
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

use super::Vec3D;

/// A vector in 3D space.
///
/// ```
/// use re_log_types::field_types::LineStrip3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     LineStrip3D::data_type(),
///     DataType::List(Box::new(Field::new(
///        "item",
///        DataType::FixedSizeList(Box::new(Field::new("item", DataType::Float32, false)), 3),
///        false,
///     )))
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct LineStrip3D(Vec<Vec3D>);

impl From<Vec<[f32; 3]>> for LineStrip3D {
    fn from(v: Vec<[f32; 3]>) -> Self {
        Self(v.into_iter().map(Vec3D).collect())
    }
}

impl Component for LineStrip3D {
    fn name() -> crate::ComponentName {
        "rerun.linestrip3d".into()
    }
}

arrow_enable_vec_for_type!(LineStrip3D);

impl ArrowField for LineStrip3D {
    type Type = Self;
    fn data_type() -> DataType {
        <Vec<Vec3D> as ArrowField>::data_type()
        /*

        */
    }
}

impl ArrowSerialize for LineStrip3D {
    // Arrow2-convert barfs on `<Vec<Vec3D> as ArrowSerialize>::MutableArrayType`
    // So do this one from scratch
    type MutableArrayType =
        arrow2::array::MutableListArray<i32, MutableFixedSizeListArray<MutablePrimitiveArray<f32>>>;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        let primitive = MutablePrimitiveArray::<f32>::new();
        // Note: we have to use new_with_field instead of new since new() defaults to nullable fields
        let vals = MutableFixedSizeListArray::<MutablePrimitiveArray<f32>>::new_with_field(
            primitive, "item", false, 3,
        );
        Self::MutableArrayType::new_from(vals, LineStrip3D::data_type(), 0)
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        let values = array.mut_values();
        let primitives = values.mut_values();
        for vec in &v.0 {
            primitives.extend_from_slice(vec.0.as_slice());
        }
        for _ in 0..v.0.len() {
            values.try_push_valid().ok();
        }
        array.try_push_valid()
    }
}

impl ArrowDeserialize for LineStrip3D {
    type ArrayType = <Vec<Vec3D> as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <Vec<Vec3D> as ArrowDeserialize>::arrow_deserialize(v).map(Self)
    }
}

#[test]
fn test_line3d_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let lines_in: Vec<LineStrip3D> = vec![
        vec![
            [1.0, 2.0, 3.0],
            [4.0, 5.0, 6.0],
            [7.0, 8.0, 9.0],
            [10.0, 11.0, 12.0],
        ]
        .into(),
        vec![[13.0, 14.0, 15.0], [16.0, 17.0, 18.0]].into(),
    ];
    let array: Box<dyn Array> = lines_in.try_into_arrow().unwrap();
    let lines_out: Vec<LineStrip3D> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(lines_in, lines_out);
}
