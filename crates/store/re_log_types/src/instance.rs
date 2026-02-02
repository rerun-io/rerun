/// A unique numeric index for each individual instance within a batch.
///
/// Use [`Instance::ALL`] to refer to all instances in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Instance(pub(crate) u64);

impl From<u64> for Instance {
    #[inline]
    fn from(instance: u64) -> Self {
        if cfg!(debug_assertions) && instance == u64::MAX {
            re_log::warn!(
                "u64::MAX is reserved to refer to all instances: {:#?}",
                std::backtrace::Backtrace::capture()
            );
        }
        Self(instance)
    }
}

impl Instance {
    /// Refer to all instances in a batch.
    pub const ALL: Self = Self(u64::MAX);

    #[inline]
    pub fn get(self) -> u64 {
        self.0
    }

    #[expect(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = impl Into<Self>>) -> Vec<Self> {
        it.into_iter().map(Into::into).collect::<Vec<_>>()
    }

    /// Are we referring to all instances of the entity (e.g. all points in a point cloud entity)?
    ///
    /// The opposite of [`Self::is_specific`].
    #[inline]
    pub fn is_all(self) -> bool {
        self == Self::ALL
    }

    /// Are we referring to a specific instance of the entity (e.g. a specific point in a point cloud)?
    ///
    /// The opposite of [`Self::is_all`].
    #[inline]
    pub fn is_specific(self) -> bool {
        self != Self::ALL
    }

    /// Returns `None` if `ALL`, otherwise the index.
    #[inline]
    pub fn specific_index(self) -> Option<Self> {
        self.is_specific().then_some(self)
    }

    /// Creates a new [`Instance`] that identifies a 2D coordinate.
    #[inline]
    pub fn from_2d_image_coordinate([x, y]: [u32; 2], image_width: u64) -> Self {
        Self((x as u64) + (y as u64) * image_width)
    }

    /// Retrieves 2D image coordinates (x, y) encoded in an instance key
    #[inline]
    pub fn to_2d_image_coordinate(self, image_width: u32) -> [u32; 2] {
        [
            (self.0 % image_width as u64) as u32,
            (self.0 / image_width as u64) as u32,
        ]
    }
}

impl std::fmt::Display for Instance {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_all() {
            "<all>".fmt(f)
        } else {
            re_format::format_uint(self.0).fmt(f)
        }
    }
}
