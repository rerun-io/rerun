use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// A Radius component
///
/// ```
/// use re_log_types::field_types::Radius;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Radius::data_type(), DataType::F32);
/// ```
#[derive(Debug)]
pub struct Radius(pub f32);

arrow_enable_vec_for_type!(Radius);

impl ArrowField for Radius {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        <f32 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Radius {
    type MutableArrayType = <f32 as ArrowSerialize>::MutableArrayType;

    fn new_array() -> Self::MutableArrayType {
        <f32 as ArrowSerialize>::new_array()
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        <f32 as ArrowSerialize>::arrow_serialize(&v.0, array)
    }
}

impl ArrowDeserialize for Radius {
    type ArrayType = <f32 as ArrowDeserialize>::ArrayType;

    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <f32 as ArrowDeserialize>::arrow_deserialize(v).map(Radius)
    }
}

impl Component for Radius {
    fn name() -> crate::ComponentName {
        "rerun.radius".into()
    }
}
