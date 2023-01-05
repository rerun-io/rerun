use arrow2::{array::PrimitiveArray, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type,
    deserialize::ArrowDeserialize,
    field::{ArrowField, FixedSizeVec},
    serialize::ArrowSerialize,
};

use super::Vec3D;

/// A 3x3 Matrix made up of 3 Vecs
///
/// ```
/// use re_log_types::field_types::Mat3x3;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Mat3x3::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         9
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mat3x3([Vec3D; 3]);

impl<Idx> std::ops::Index<Idx> for Mat3x3
where
    Idx: std::slice::SliceIndex<[Vec3D]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl From<[[f32; 3]; 3]> for Mat3x3 {
    fn from(v: [[f32; 3]; 3]) -> Self {
        Self([Vec3D(v[0]), Vec3D(v[1]), Vec3D(v[2])])
    }
}

arrow_enable_vec_for_type!(Mat3x3);

impl ArrowField for Mat3x3 {
    type Type = Self;
    fn data_type() -> DataType {
        <FixedSizeVec<f32, 9> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Mat3x3 {
    type MutableArrayType = <FixedSizeVec<f32, 9> as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        FixedSizeVec::<f32, 9>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        for col in v.0 {
            array.mut_values().extend_from_slice(&col.0);
        }
        array.try_push_valid()
    }
}

impl ArrowDeserialize for Mat3x3 {
    type ArrayType = <FixedSizeVec<f32, 2> as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.map(|v| {
            let slice = v
                .as_any()
                .downcast_ref::<PrimitiveArray<f32>>()
                .unwrap()
                .values()
                .as_slice();
            Mat3x3([
                Vec3D(slice[0..3].try_into().unwrap()),
                Vec3D(slice[3..6].try_into().unwrap()),
                Vec3D(slice[6..9].try_into().unwrap()),
            ])
        })
    }
}
