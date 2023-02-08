use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A 16-bit ID representing a type of semantic keypoint within a class.
///
/// `KeypointId`s are only meaningful within the context of a [`crate::context::ClassDescription`].
///
/// Used to look up an [`crate::context::AnnotationInfo`] for a Keypoint within the [`crate::context::AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[arrow_field(transparent)]
pub struct KeypointId(pub u16);

impl Component for KeypointId {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.keypoint_id".into()
    }
}
