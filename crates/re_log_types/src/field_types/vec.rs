use arrow2::{array::PrimitiveArray, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type,
    deserialize::ArrowDeserialize,
    field::{ArrowField, FixedSizeVec},
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// A vector in 2D space.
///
/// ```
/// use re_log_types::field_types::Vec2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Vec2D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         2
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vec2D(pub [f32; 2]);

impl Vec2D {
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }
}

impl From<[f32; 2]> for Vec2D {
    fn from(v: [f32; 2]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec2D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl Component for Vec2D {
    fn name() -> crate::ComponentName {
        "rerun.vec2d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec2D> for glam::Vec2 {
    fn from(v: Vec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Vec2D {
    fn from(v: glam::Vec2) -> Self {
        Self(v.to_array())
    }
}

arrow_enable_vec_for_type!(Vec2D);

impl ArrowField for Vec2D {
    type Type = Self;
    fn data_type() -> DataType {
        <FixedSizeVec<f32, 2> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Vec2D {
    type MutableArrayType = <FixedSizeVec<f32, 2> as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        FixedSizeVec::<f32, 2>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.mut_values().extend_from_slice(&v.0);
        array.try_push_valid()
    }
}

impl ArrowDeserialize for Vec2D {
    type ArrayType = <FixedSizeVec<f32, 2> as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.map(|v| {
            Vec2D(
                v.as_any()
                    .downcast_ref::<PrimitiveArray<f32>>()
                    .unwrap()
                    .values()
                    .as_slice()
                    .try_into()
                    .unwrap(),
            )
        })
    }
}

/// A vector in 3D space.
///
/// ```
/// use re_log_types::field_types::Vec3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Vec3D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         3
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vec3D(pub [f32; 3]);

impl Vec3D {
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }
}

impl From<[f32; 3]> for Vec3D {
    fn from(v: [f32; 3]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec3D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl Component for Vec3D {
    fn name() -> crate::ComponentName {
        "rerun.vec3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec3D> for glam::Vec3 {
    fn from(v: Vec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3D {
    fn from(v: glam::Vec3) -> Self {
        Self(v.to_array())
    }
}

arrow_enable_vec_for_type!(Vec3D);

impl ArrowField for Vec3D {
    type Type = Self;
    fn data_type() -> DataType {
        <FixedSizeVec<f32, 3> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Vec3D {
    type MutableArrayType = <FixedSizeVec<f32, 3> as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        FixedSizeVec::<f32, 3>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.mut_values().extend_from_slice(&v.0);
        array.try_push_valid()
    }
}

impl ArrowDeserialize for Vec3D {
    type ArrayType = <FixedSizeVec<f32, 3> as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.map(|v| {
            Vec3D(
                v.as_any()
                    .downcast_ref::<PrimitiveArray<f32>>()
                    .unwrap()
                    .values()
                    .as_slice()
                    .try_into()
                    .unwrap(),
            )
        })
    }
}
