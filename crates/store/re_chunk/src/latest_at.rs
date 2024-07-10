use arrow2::array::Array as ArrowArray;

use re_log_types::{TimeInt, Timeline};
use re_types_core::ComponentName;

use crate::{Chunk, RowId};

// ---

/// A query at a given time, for a given timeline.
///
/// Get the latest version of the data available at this time.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LatestAtQuery {
    timeline: Timeline,
    at: TimeInt,
}

impl std::fmt::Debug for LatestAtQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<latest at {} on {:?}>",
            self.timeline.typ().format_utc(self.at),
            self.timeline.name(),
        ))
    }
}

impl LatestAtQuery {
    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn new(timeline: Timeline, at: impl TryInto<TimeInt>) -> Self {
        let at = at.try_into().unwrap_or(TimeInt::MIN);
        Self { timeline, at }
    }

    #[inline]
    pub const fn latest(timeline: Timeline) -> Self {
        Self {
            timeline,
            at: TimeInt::MAX,
        }
    }

    #[inline]
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    #[inline]
    pub fn at(&self) -> TimeInt {
        self.at
    }
}

// ---

impl Chunk {
    /// Runs a [`LatestAtQuery`] filter on a [`Chunk`].
    ///
    /// This behaves as a row-based filter: the result is a new [`Chunk`] that is vertically
    /// sliced to only contain the row relevant for the specified `query`.
    ///
    /// The resulting [`Chunk`] is guaranteed to contain all the same columns has the queried
    /// chunk: there is no horizontal slicing going on.
    ///
    /// An empty [`Chunk`] (i.e. 0 rows, but N columns) is returned if the `query` yields nothing.
    ///
    /// Because the resulting chunk doesn't discard any column information, you can find extra relevant
    /// information by inspecting the data, for examples timestamps on other timelines.
    /// See [`Self::timeline_sliced`] and [`Self::component_sliced`] if you do want to filter this
    /// extra data.
    pub fn latest_at(&self, query: &LatestAtQuery, component_name: ComponentName) -> Self {
        if self.is_empty() {
            return self.clone();
        }

        re_tracing::profile_function!(format!("{query:?}"));

        let Some(component_list_array) = self.components.get(&component_name) else {
            return self.emptied();
        };

        let mut index = None;

        let is_static = self.is_static();
        let is_sorted_by_row_id = self.is_sorted();

        if is_static {
            if is_sorted_by_row_id {
                // Static, row-sorted chunk

                for i in (0..self.num_rows()).rev() {
                    if !component_list_array.is_valid(i) {
                        continue;
                    }

                    index = Some(i);
                    break;
                }
            } else {
                // Static, row-unsorted chunk

                let mut closest_row_id = RowId::ZERO;

                for (i, row_id) in self.row_ids().enumerate() {
                    if !component_list_array.is_valid(i) {
                        continue;
                    }

                    let is_closer_row_id = row_id > closest_row_id;

                    if is_closer_row_id {
                        closest_row_id = row_id;
                        index = Some(i);
                    }
                }
            }
        } else {
            let Some(time_chunk) = self.timelines.get(&query.timeline()) else {
                return self.emptied();
            };

            let is_sorted_by_time = time_chunk.is_sorted();
            let times = time_chunk.times_raw();

            if is_sorted_by_time {
                // Temporal, row-sorted, time-sorted chunk

                let i = times
                    .partition_point(|&time| time <= query.at().as_i64())
                    .saturating_sub(1);

                for i in (0..=i).rev() {
                    if !component_list_array.is_valid(i) {
                        continue;
                    }

                    index = Some(i);
                    break;
                }
            } else {
                // Temporal, unsorted chunk

                let mut closest_data_time = TimeInt::MIN;
                let mut closest_row_id = RowId::ZERO;

                for (i, row_id) in self.row_ids().enumerate() {
                    if !component_list_array.is_valid(i) {
                        continue;
                    }

                    let data_time = TimeInt::new_temporal(times[i]);

                    let is_closer_time = data_time > closest_data_time && data_time <= query.at();
                    let is_same_time_but_closer_row_id =
                        data_time == closest_data_time && row_id > closest_row_id;

                    if is_closer_time || is_same_time_but_closer_row_id {
                        closest_data_time = data_time;
                        closest_row_id = row_id;
                        index = Some(i);
                    }
                }
            }
        }

        index.map_or_else(|| self.emptied(), |i| self.row_sliced(i, 1))
    }
}
