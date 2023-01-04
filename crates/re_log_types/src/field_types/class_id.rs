use arrow2::array::TryPush;
use arrow2_convert::{deserialize::ArrowDeserialize, field::ArrowField, serialize::ArrowSerialize};

use crate::msg_bundle::Component;

/// A 16-bit ID representing a type of semantic class.
///
/// Used to look up a [`crate::context::ClassDescription`] within the [`crate::context::AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassId(pub u16);

impl ArrowField for ClassId {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        <u16 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for ClassId {
    type MutableArrayType = <u16 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for ClassId {
    type ArrayType = <u16 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u16 as ArrowDeserialize>::arrow_deserialize(v).map(ClassId)
    }
}

impl Component for ClassId {
    fn name() -> crate::ComponentName {
        "rerun.class_id".into()
    }
}
