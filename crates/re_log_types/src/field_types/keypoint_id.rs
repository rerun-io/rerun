use arrow2::array::TryPush;
use arrow2_convert::{deserialize::ArrowDeserialize, field::ArrowField, serialize::ArrowSerialize};

use crate::msg_bundle::Component;

/// A 16-bit ID representing a type of semantic keypoint within a class.
///
/// `KeypointId`s are only meaningful within the context of a [`crate::context::ClassDescription`].
///
/// Used to look up an [`crate::context::AnnotationInfo`] for a Keypoint within the [`crate::context::AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeypointId(pub u16);

impl ArrowField for KeypointId {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        <u16 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for KeypointId {
    type MutableArrayType = <u16 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        <u16 as ArrowSerialize>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for KeypointId {
    type ArrayType = <u16 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u16 as ArrowDeserialize>::arrow_deserialize(v).map(KeypointId)
    }
}

impl Component for KeypointId {
    fn name() -> crate::ComponentName {
        "rerun.keypoint_id".into()
    }
}
