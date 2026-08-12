use arrow::array::Array as _;
use re_log_types::{TimeInt, TimelineName};
use re_types_core::ComponentIdentifier;

use crate::{Chunk, RowId, UnitChunkShared};

// ---

/// A query at a given time, for a given timeline.
///
/// Get the latest version of the data available at this time.
///
/// The timeline is `None` for a static-only query (see [`Self::new_static`]), where no timeline
/// is relevant.
#[derive(Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub struct LatestAtQuery {
    timeline: Option<TimelineName>,

    /// The time being queried, or [`TimeInt::STATIC`] for a static-only query.
    at: TimeInt,
}

impl std::fmt::Debug for LatestAtQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.timeline {
            Some(timeline) => {
                f.write_fmt(format_args!("<latest-at {:?} on {:?}>", self.at, timeline))
            }
            None => f.write_fmt(format_args!("<latest-at {:?} (static)>", self.at)),
        }
    }
}

impl LatestAtQuery {
    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn new(timeline: TimelineName, at: impl TryInto<TimeInt>) -> Self {
        Self {
            timeline: Some(timeline),
            at: TimeInt::saturated_temporal(at),
        }
    }

    #[inline]
    pub const fn latest(timeline: TimelineName) -> Self {
        Self {
            timeline: Some(timeline),
            at: TimeInt::MAX,
        }
    }

    /// A query for static data only, where no timeline is relevant.
    #[inline]
    pub const fn new_static() -> Self {
        Self {
            timeline: None,
            at: TimeInt::STATIC,
        }
    }

    /// The timeline being queried, or `None` for a static-only query.
    #[inline]
    pub fn timeline(&self) -> Option<TimelineName> {
        self.timeline
    }

    /// The time being queried, or [`TimeInt::STATIC`] for a static-only query.
    #[inline]
    pub fn at(&self) -> TimeInt {
        self.at
    }
}

// ---

impl Chunk {
    /// Runs a [`LatestAtQuery`] filter on a [`Chunk`].
    ///
    /// This behaves as a row-based filter: the result is a [`UnitChunkShared`] that is vertically
    /// sliced to only contain the row relevant for the specified `query`.
    ///
    /// The resulting chunk is guaranteed to contain all the same columns as the queried
    /// chunk: there is no horizontal slicing going on.
    ///
    /// Returns `None` if the `query` yields nothing.
    ///
    /// Because the resulting chunk doesn't discard any column information, you can find extra relevant
    /// information by inspecting the data, for examples timestamps on other timelines.
    /// See [`Self::timeline_sliced`] and [`Self::component_sliced`] if you do want to filter this
    /// extra data.
    pub fn latest_at(
        &self,
        query: &LatestAtQuery,
        component: ComponentIdentifier,
    ) -> Option<UnitChunkShared> {
        if self.is_empty() {
            return None;
        }

        re_tracing::profile_function!(format!("{query:?}"));

        let component_list_array = self.components.get_array(component)?;

        let mut index = None;

        let is_static = self.is_static();
        let is_sorted_by_row_id = self.is_row_ids_sorted();

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
            let time_column = self.timelines.get(&query.timeline()?)?;

            let is_sorted_by_time = time_column.is_sorted();
            let times = time_column.times_raw();

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

        index.map(|i| self.row_sliced_unit_shallow(i))
    }
}
