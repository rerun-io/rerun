use re_data_store::{TimeInt, TimeRange};

// ---

/// One of the boundaries of the visible history.
///
/// For [`VisibleHistoryBoundary::RelativeToTimeCursor`] and [`VisibleHistoryBoundary::Absolute`],
/// the value are either nanos or frames, depending on the type of timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum VisibleHistoryBoundary {
    /// Boundary is a value relative to the time cursor
    RelativeToTimeCursor(i64),

    /// Boundary is an absolute value
    Absolute(i64),

    /// The boundary extends to infinity.
    Infinite,
}

impl VisibleHistoryBoundary {
    /// Value when the boundary is set to the current time cursor.
    pub const AT_CURSOR: Self = Self::RelativeToTimeCursor(0);
}

impl Default for VisibleHistoryBoundary {
    fn default() -> Self {
        Self::AT_CURSOR
    }
}

/// Visible history bounds.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct VisibleHistory {
    /// Low time boundary.
    pub from: VisibleHistoryBoundary,

    /// High time boundary.
    pub to: VisibleHistoryBoundary,
}

impl VisibleHistory {
    /// Value with the visible history feature is disabled.
    pub const OFF: Self = Self {
        from: VisibleHistoryBoundary::AT_CURSOR,
        to: VisibleHistoryBoundary::AT_CURSOR,
    };

    pub const ALL: Self = Self {
        from: VisibleHistoryBoundary::Infinite,
        to: VisibleHistoryBoundary::Infinite,
    };

    /// Returns the start boundary of the time range given an input cursor position.
    ///
    /// This is not guaranteed to be lesser than or equal to [`Self::to`].
    /// Do not use this to build a [`TimeRange`], use [`Self::time_range`].
    #[doc(hidden)]
    pub fn range_start_from_cursor(&self, cursor: TimeInt) -> TimeInt {
        match self.from {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::new_temporal(value),
            VisibleHistoryBoundary::RelativeToTimeCursor(value) => {
                cursor + TimeInt::new_temporal(value)
            }
            VisibleHistoryBoundary::Infinite => TimeInt::MIN,
        }
    }

    /// Returns the end boundary of the time range given an input cursor position.
    ///
    /// This is not guaranteed to be greater than [`Self::from`].
    /// Do not use this to build a [`TimeRange`], use [`Self::time_range`].
    #[doc(hidden)]
    pub fn range_end_from_cursor(&self, cursor: TimeInt) -> TimeInt {
        match self.to {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::new_temporal(value),
            VisibleHistoryBoundary::RelativeToTimeCursor(value) => {
                cursor + TimeInt::new_temporal(value)
            }
            VisibleHistoryBoundary::Infinite => TimeInt::MAX,
        }
    }

    /// Returns a _sanitized_ [`TimeRange`], i.e. guaranteed to be monotonically increasing.
    pub fn time_range(&self, cursor: TimeInt) -> TimeRange {
        let mut from = self.range_start_from_cursor(cursor);
        let mut to = self.range_end_from_cursor(cursor);

        // TODO(#4993): visible time range UI can yield inverted ranges
        if from > to {
            std::mem::swap(&mut from, &mut to);
        }

        TimeRange::new(from, to)
    }
}

/// When showing an entity in the history view, add this much history to it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ExtraQueryHistory {
    /// Is the feature enabled?
    pub enabled: bool,

    /// Visible history settings for time timelines
    pub nanos: VisibleHistory,

    /// Visible history settings for frame timelines
    pub sequences: VisibleHistory,
}

impl ExtraQueryHistory {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        if child.enabled {
            *child
        } else if self.enabled {
            *self
        } else {
            Self::default()
        }
    }
}
