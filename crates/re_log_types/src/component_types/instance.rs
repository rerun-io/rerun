use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// The Instance used to identify an entity within a batch
///
/// ```
/// use re_log_types::component_types::Instance;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Instance::data_type(), DataType::UInt64);
/// ```
#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Instance(pub u64);

impl Instance {
    pub const SPLAT: Self = Self(u64::MAX);

    #[inline]
    pub fn is_splat(&self) -> bool {
        self.0 == u64::MAX
    }
}

impl std::fmt::Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            let key = self.0;
            format!("key:{key}").fmt(f)
        }
    }
}

impl Component for Instance {
    fn name() -> crate::ComponentName {
        "rerun.instance".into()
    }
}
