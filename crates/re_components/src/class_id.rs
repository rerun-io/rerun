use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

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

impl re_log_types::LegacyComponent for ClassId {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.class_id".into()
    }
}

impl From<re_types::components::ClassId> for ClassId {
    fn from(other: re_types::components::ClassId) -> Self {
        Self(other.0)
    }
}

re_log_types::component_legacy_shim!(ClassId);
