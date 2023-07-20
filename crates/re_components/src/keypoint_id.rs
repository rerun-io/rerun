use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

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

impl re_log_types::Component for KeypointId {
    #[inline]
    fn name() -> re_log_types::ComponentName {
        "rerun.keypoint_id".into()
    }
}

impl From<re_types::components::KeypointId> for KeypointId {
    fn from(other: re_types::components::KeypointId) -> Self {
        Self(other.0)
    }
}
