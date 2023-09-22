use super::DrawOrder;

// TODO(cmc): come up with some DSL in our flatbuffers definitions so that we can declare these
// constants directly in there.
impl DrawOrder {
    /// Draw order used for images if no draw order was specified.
    pub const DEFAULT_IMAGE: DrawOrder = DrawOrder(-10.0);

    /// Draw order used for 2D boxes if no draw order was specified.
    pub const DEFAULT_BOX2D: DrawOrder = DrawOrder(10.0);

    /// Draw order used for 2D lines if no draw order was specified.
    pub const DEFAULT_LINES2D: DrawOrder = DrawOrder(20.0);

    /// Draw order used for 2D points if no draw order was specified.
    pub const DEFAULT_POINTS2D: DrawOrder = DrawOrder(30.0);
}

impl std::cmp::PartialEq for DrawOrder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.is_nan() && other.0.is_nan() || self.0 == other.0
    }
}

impl std::cmp::Eq for DrawOrder {}

impl std::cmp::PartialOrd for DrawOrder {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if other == self {
            Some(std::cmp::Ordering::Equal)
        } else if other.0.is_nan() || self.0 < other.0 {
            Some(std::cmp::Ordering::Less)
        } else {
            Some(std::cmp::Ordering::Greater)
        }
    }
}

impl std::cmp::Ord for DrawOrder {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}
