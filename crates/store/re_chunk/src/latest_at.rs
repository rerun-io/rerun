use arrow::array::{Array as _, ListArray as ArrowListArray};
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

/// Which end of the [`RowId`] index a query resolves a static chunk to.
///
/// [`RowId`] is an index like any other: an earliest-at query looks for the lowest one, a
/// latest-at query for the highest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowIdEnd {
    /// The lowest [`RowId`].
    Earliest,

    /// The highest [`RowId`].
    Latest,
}

impl Chunk {
    /// Runs a [`LatestAtQuery`] filter on a [`Chunk`].
    ///
    /// This behaves as a row-based filter: the result is a [`UnitChunkShared`] that is vertically
    /// sliced to only contain the row relevant for the specified `query`.
    ///
    /// The resulting chunk is guaranteed to contain all the same columns as the queried
    /// chunk: there is no horizontal slicing going on.
    ///
    /// Ties on the query time are broken on the highest [`RowId`].
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

        let index = if self.is_static() {
            self.static_row_index(component_list_array, RowIdEnd::Latest)
        } else {
            let time_column = self.timelines.get(&query.timeline()?)?;

            let is_sorted_by_time = time_column.is_sorted();
            let times = time_column.times_raw();

            let mut index = None;

            if is_sorted_by_time {
                // Temporal, time-sorted chunk

                // One past the last row whose time is at-or-before the query time. Zero when the
                // query precedes every row, which must answer nothing rather than the first row.
                let end = times.partition_point(|&time| time <= query.at().as_i64());

                if self.is_row_ids_sorted() {
                    // Row-ids ascend within each run of equal times, so the first valid row
                    // walking back already holds the highest `RowId` of its run.
                    index = (0..end).rev().find(|&i| component_list_array.is_valid(i));
                } else {
                    // The first valid row walking back settles the data time, but the rest of that
                    // time's run can still hold a higher `RowId`, which wins the tie.
                    let row_ids = self.row_ids_slice();
                    let mut best_row_id = RowId::ZERO;

                    for i in (0..end).rev() {
                        if !component_list_array.is_valid(i) {
                            continue;
                        }

                        match index {
                            None => {
                                best_row_id = row_ids[i];
                                index = Some(i);
                            }
                            Some(best) => {
                                if times[i] != times[best] {
                                    break;
                                }
                                if row_ids[i] > best_row_id {
                                    best_row_id = row_ids[i];
                                    index = Some(i);
                                }
                            }
                        }
                    }
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

            index
        };

        index.map(|i| self.row_sliced_unit_shallow(i))
    }

    /// The row index holding the valid value at one end of the [`RowId`] index, for the given
    /// component array, in a static chunk.
    ///
    /// Static data has no temporal ordering, so [`RowId`] is the only index left to search along.
    /// `end` picks which way to search it, the same way the query time picks a direction on a
    /// temporal chunk.
    pub(crate) fn static_row_index(
        &self,
        component_list_array: &ArrowListArray,
        end: RowIdEnd,
    ) -> Option<usize> {
        if self.is_row_ids_sorted() {
            // Static, row-sorted chunk
            match end {
                RowIdEnd::Earliest => {
                    (0..self.num_rows()).find(|&i| component_list_array.is_valid(i))
                }
                RowIdEnd::Latest => (0..self.num_rows())
                    .rev()
                    .find(|&i| component_list_array.is_valid(i)),
            }
        } else {
            // Static, row-unsorted chunk
            //
            // `None` until the first valid row: `RowId::ZERO` is a floor, not a ceiling, so it
            // cannot seed a search for the lowest `RowId`.
            let mut best: Option<(RowId, usize)> = None;

            for (i, row_id) in self.row_ids().enumerate() {
                if !component_list_array.is_valid(i) {
                    continue;
                }

                let is_better = match best {
                    None => true,
                    Some((best_row_id, _)) => match end {
                        RowIdEnd::Earliest => row_id < best_row_id,
                        RowIdEnd::Latest => row_id > best_row_id,
                    },
                };

                if is_better {
                    best = Some((row_id, i));
                }
            }

            best.map(|(_, i)| i)
        }
    }
}
