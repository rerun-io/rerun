use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::LegacyComponent;

/// A number used to specify a specific instance in an entity.
///
/// Each entity can have many component of the same type.
/// These are identified with [`InstanceKey`].
///
/// This is a special component type. All entities has this component, at least implicitly.
///
/// For instance: A point cloud is one entity, and each point is an instance, idenitifed by an [`InstanceKey`].
///
/// ```
/// use re_log_types::LegacyInstanceKey;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(LegacyInstanceKey::data_type(), DataType::UInt64);
/// ```
#[derive(
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, ArrowField, ArrowSerialize, ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct LegacyInstanceKey(pub u64);

impl LegacyInstanceKey {
    /// A special value indicating that this [`LegacyInstanceKey]` is referring to all instances of an entity,
    /// for example all points in a point cloud entity.
    pub const SPLAT: Self = Self(u64::MAX);

    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = impl Into<Self>>) -> Vec<Self> {
        it.into_iter().map(Into::into).collect::<Vec<_>>()
    }

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
    #[inline]
    pub fn specific_index(self) -> Option<LegacyInstanceKey> {
        self.is_specific().then_some(self)
    }

    /// Creates a new [`LegacyInstanceKey`] that identifies a 2d coordinate.
    pub fn from_2d_image_coordinate([x, y]: [u32; 2], image_width: u64) -> Self {
        Self((x as u64) + (y as u64) * image_width)
    }

    /// Retrieves 2d image coordinates (x, y) encoded in an instance key
    pub fn to_2d_image_coordinate(self, image_width: u64) -> [u32; 2] {
        [(self.0 % image_width) as u32, (self.0 / image_width) as u32]
    }
}

impl std::fmt::Debug for LegacyInstanceKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            self.0.fmt(f)
        }
    }
}

impl std::fmt::Display for LegacyInstanceKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            self.0.fmt(f)
        }
    }
}

impl From<u64> for LegacyInstanceKey {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl LegacyComponent for LegacyInstanceKey {
    #[inline]
    fn legacy_name() -> crate::ComponentName {
        "rerun.instance_key".into()
    }
}

impl From<re_types::components::InstanceKey> for LegacyInstanceKey {
    fn from(other: re_types::components::InstanceKey) -> Self {
        Self(other.0)
    }
}

impl From<LegacyInstanceKey> for re_types::components::InstanceKey {
    fn from(other: LegacyInstanceKey) -> Self {
        Self(other.0)
    }
}

use crate as re_log_types;

re_log_types::component_legacy_shim!(LegacyInstanceKey);
