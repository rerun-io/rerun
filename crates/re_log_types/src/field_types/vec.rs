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
pub struct Vec2D {
    pub x: f32,
    pub y: f32,
}

impl Component for Vec2D {
    fn name() -> crate::ComponentName {
        "rerun.vec2d".into()
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
        array.mut_values().extend_from_slice(&[v.x, v.y]);
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
            let v = v
                .as_any()
                .downcast_ref::<PrimitiveArray<f32>>()
                .unwrap()
                .values()
                .as_slice();
            Vec2D { x: v[0], y: v[1] }
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
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vec3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Vec3D {
    fn name() -> crate::ComponentName {
        "rerun.vec3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec3D> for glam::Vec3 {
    fn from(v: Vec3D) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3D {
    fn from(v: glam::Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
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
        array.mut_values().extend_from_slice(&[v.x, v.y, v.z]);
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
            let v = v
                .as_any()
                .downcast_ref::<PrimitiveArray<f32>>()
                .unwrap()
                .values()
                .as_slice();
            Vec3D {
                x: v[0],
                y: v[1],
                z: v[2],
            }
        })
    }
}
