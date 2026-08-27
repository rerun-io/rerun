use arrow::array::Array as _;
use re_log_types::{TimeInt, TimelineName};
use re_types_core::ComponentIdentifier;

use crate::{Chunk, RowId, UnitChunkShared, latest_at::RowIdEnd};

// ---

/// A query at a given time, for a given timeline.
///
/// Get the earliest version of the data available at-or-after this time.
///
/// The timeline is `None` for a static-only query (see [`Self::new_static`]), where no timeline
/// is relevant.
#[derive(Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub struct EarliestAtQuery {
    timeline: Option<TimelineName>,

    /// The time being queried, or [`TimeInt::STATIC`] for a static-only query.
    at: TimeInt,
}

impl std::fmt::Debug for EarliestAtQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.timeline {
            Some(timeline) => f.write_fmt(format_args!(
                "<earliest-at {:?} on {:?}>",
                self.at, timeline
            )),
            None => f.write_fmt(format_args!("<earliest-at {:?} (static)>", self.at)),
        }
    }
}

impl EarliestAtQuery {
    /// The returned query is guaranteed to never include [`TimeInt::STATIC`].
    ///
    /// To query static data, use [`Self::new_static`].
    #[inline]
    pub fn new(timeline: TimelineName, at: impl TryInto<TimeInt>) -> Self {
        // A failed conversion means a time below the representable range, which clamps to `MIN`
        // for both directions. Only `STATIC` is treated differently.
        // `STATIC` set to MAX since that's never the earliest value. Mirroring latest_at.
        let at = at.try_into().unwrap_or(TimeInt::MIN);
        Self {
            timeline: Some(timeline),
            at: if at.is_static() { TimeInt::MAX } else { at },
        }
    }

    /// Query the earliest static data, i.e. the one with lowest [`RowId`].
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
    /// Runs an [`EarliestAtQuery`] filter on a [`Chunk`].
    ///
    /// Returns the row holding the earliest value at-or-after the query time, breaking ties on
    /// the lowest [`RowId`].
    /// Returns `None` if the `query` yields nothing.
    //
    // NOTE: keep the temporal logic here in sync with `Self::latest_at`.
    pub fn earliest_at(
        &self,
        query: &EarliestAtQuery,
        component: ComponentIdentifier,
    ) -> Option<UnitChunkShared> {
        if self.is_empty() {
            return None;
        }

        re_tracing::profile_function!(format!("{query:?}"));

        let component_list_array = self.components.get_array(component)?;

        let index = if self.is_static() {
            self.static_row_index(component_list_array, RowIdEnd::Earliest)
        } else {
            let time_column = self.timelines.get(&query.timeline()?)?;

            let is_sorted_by_time = time_column.is_sorted();
            let times = time_column.times_raw();

            let mut index = None;

            if is_sorted_by_time {
                // Temporal, time-sorted chunk

                // The first row whose time is at-or-after the query time.
                let first = times.partition_point(|&time| time < query.at().as_i64());

                if self.is_row_ids_sorted() {
                    // Row-ids ascend within each run of equal times, so the first valid row from
                    // there already holds the lowest `RowId` of its run.
                    index = (first..self.num_rows()).find(|&i| component_list_array.is_valid(i));
                } else {
                    // The first valid row from there settles the data time, but the rest of that
                    // time's run can still hold a lower `RowId`, which wins the tie.
                    let row_ids = self.row_ids_slice();
                    let mut best_row_id = RowId::ZERO;

                    for i in first..self.num_rows() {
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
                                if row_ids[i] < best_row_id {
                                    best_row_id = row_ids[i];
                                    index = Some(i);
                                }
                            }
                        }
                    }
                }
            } else {
                // Temporal, unsorted chunk

                // The smallest `data_time` at-or-after the query time, with the smallest `row_id`
                // at that time. `None` until we see the first candidate row, because there is no
                // `TimeInt` sentinel that is greater than all temporal times.
                let mut best: Option<(TimeInt, RowId)> = None;

                for (i, row_id) in self.row_ids().enumerate() {
                    if !component_list_array.is_valid(i) {
                        continue;
                    }

                    let data_time = TimeInt::new_temporal(times[i]);

                    if data_time < query.at() {
                        continue;
                    }

                    let is_better = match best {
                        None => true,
                        Some((best_time, best_row_id)) => {
                            data_time < best_time
                                || (data_time == best_time && row_id < best_row_id)
                        }
                    };

                    if is_better {
                        best = Some((data_time, row_id));
                        index = Some(i);
                    }
                }
            }

            index
        };

        index.map(|i| self.row_sliced_unit_shallow(i))
    }
}
