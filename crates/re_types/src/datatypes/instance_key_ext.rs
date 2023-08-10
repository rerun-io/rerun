use super::InstanceKey;

// TODO(cmc): come up with some DSL in our flatbuffers definitions so that we can declare these
// constants directly in there.
impl InstanceKey {
    /// Draw order used for images if no draw order was specified.
    pub const SPLAT: Self = Self(u64::MAX);
}

impl From<u64> for InstanceKey {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<InstanceKey> for u64 {
    #[inline]
    fn from(value: InstanceKey) -> Self {
        value.0
    }
}

impl InstanceKey {
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
    pub fn specific_index(self) -> Option<InstanceKey> {
        self.is_specific().then_some(self)
    }

    /// Creates a new [`InstanceKey`] that identifies a 2d coordinate.
    pub fn from_2d_image_coordinate([x, y]: [u32; 2], image_width: u64) -> Self {
        Self((x as u64) + (y as u64) * image_width)
    }

    /// Retrieves 2d image coordinates (x, y) encoded in an instance key
    pub fn to_2d_image_coordinate(self, image_width: u64) -> [u32; 2] {
        [(self.0 % image_width) as u32, (self.0 / image_width) as u32]
    }
}

impl std::fmt::Display for InstanceKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            self.0.fmt(f)
        }
    }
}
