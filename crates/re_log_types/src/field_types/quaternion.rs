use arrow2::{array::PrimitiveArray, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type,
    deserialize::ArrowDeserialize,
    field::{ArrowField, FixedSizeVec},
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// A Quaternion represented by 4 real numbers.
///
/// ```
/// use re_log_types::field_types::Quaternion;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Quaternion::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         4
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Component for Quaternion {
    fn name() -> crate::ComponentName {
        "rerun.quaternion".into()
    }
}

#[cfg(feature = "glam")]
impl From<Quaternion> for glam::Quat {
    fn from(q: Quaternion) -> Self {
        Self::from_xyzw(q.x, q.y, q.z, q.w)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Quat> for Quaternion {
    fn from(q: glam::Quat) -> Self {
        let (x, y, z, w) = q.into();
        Self { x, y, z, w }
    }
}

arrow_enable_vec_for_type!(Quaternion);

impl ArrowField for Quaternion {
    type Type = Self;
    fn data_type() -> DataType {
        <FixedSizeVec<f32, 4> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Quaternion {
    type MutableArrayType = <FixedSizeVec<f32, 4> as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        FixedSizeVec::<f32, 4>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.mut_values().extend_from_slice(&[v.x, v.y, v.z, v.w]);
        array.try_push_valid()
    }
}

impl ArrowDeserialize for Quaternion {
    type ArrayType = <FixedSizeVec<f32, 4> as ArrowDeserialize>::ArrayType;

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
            Quaternion {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
            }
        })
    }
}
