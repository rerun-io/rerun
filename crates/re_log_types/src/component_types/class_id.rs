use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A 16-bit ID representing a type of semantic class.
///
/// Used to look up a [`crate::context::ClassDescription`] within the [`crate::context::AnnotationContext`].
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
pub struct ClassId(pub u16);

impl Component for ClassId {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.class_id".into()
    }
}
