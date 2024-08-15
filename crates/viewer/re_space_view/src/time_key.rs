use re_chunk_store::RowId;
use re_log_types::TimeInt;

/// Time and row id for some piece of data.
///
/// The `Ord` for this ignores `row_id`.
/// This is so that our zipping iterators will ignore the row id when comparing.
/// This is important for static data, especially when it comes to overrides.
/// If you override some static data (e.g. point cloud radius), the row id should be ignored,
/// otherwise the zipping iterators will say the override came _after_ the original data,
/// and so by latest-at semantics, the override will be ignored.
#[derive(Clone, Copy, Debug)]
pub struct TimeKey {
    pub time: TimeInt,
    pub row_id: RowId,
}

impl From<(TimeInt, RowId)> for TimeKey {
    #[inline]
    fn from((time, row_id): (TimeInt, RowId)) -> Self {
        Self { time, row_id }
    }
}

impl PartialEq for TimeKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for TimeKey {}

impl PartialOrd for TimeKey {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.time.cmp(&other.time))
    }
}

impl Ord for TimeKey {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}
