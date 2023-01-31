use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A number used to specify a specific instance in an entity.
///
/// Each entity can have many component of the same type.
/// These are identified with [`Instance`].
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
    /// A special value indicating that this [`Instance]` is referring to all instances of an entity,
    /// for example all points in a point cloud entity.
    pub const SPLAT: Self = Self(u64::MAX);

    /// Are we referring to all instances of the entity (e.g. all points in a point cloud entity)?
    ///
    /// The opposite of [`Self::is_specific`].
    #[inline]
    pub fn is_splat(self) -> bool {
        self == Self::SPLAT
    }

    /// Are we referring to a specific instance of the entity (e.g. a specific point in a point cloud)?
    ///
    /// The opposite of [`Self::is_splat`].
    #[inline]
    pub fn is_specific(self) -> bool {
        self != Self::SPLAT
    }

    /// Returns `None` if splat, otherwise the index.
    pub fn specific_index(self) -> Option<Instance> {
        self.is_specific().then_some(self)
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
