use arrow2::{array::PrimitiveArray, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type,
    deserialize::ArrowDeserialize,
    field::{ArrowField, FixedSizeVec},
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// A 3D bounding box represented by it's half-lengths
///
/// ```
/// use re_log_types::component_types::Box3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Box3D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(
///             Field::new("item", DataType::Float32, false)
///         ),
///     3)
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Box3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Box3D {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl Component for Box3D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.box3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Box3D> for glam::Vec3 {
    #[inline]
    fn from(b: Box3D) -> Self {
        Self::new(b.x, b.y, b.z)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Box3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        let (x, y, z) = v.into();
        Self { x, y, z }
    }
}

arrow_enable_vec_for_type!(Box3D);

impl ArrowField for Box3D {
    type Type = Self;

    #[inline]
    fn data_type() -> DataType {
        <FixedSizeVec<f32, 3> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Box3D {
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

impl ArrowDeserialize for Box3D {
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
            Box3D {
                x: v[0],
                y: v[1],
                z: v[2],
            }
        })
    }
}
