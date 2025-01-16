use arrow2::array::{Array as Arrow2Array, ListArray as Arrow2ListArray};

use re_log_types::{TimeInt, Timeline};
use re_types_core::ComponentDescriptor;

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
            "<latest-at {} on {:?}>",
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

// TODO: reminder that these kinds of collisions are pretty rare anyhow.

impl Chunk {
    // TODO: now the question is... do we dispatch according to wildcard semantics already?

    // TODO: an enforced exact-match impl would be nice, possibly?
    // I.e.:
    // * latest_at: fallback/wildcard semantics (most specific wins, given initial preconditions)
    // * latest_at_exact_match: no fallback
    // * latest_at_most_specific: implementation detail of `latest_at`

    // TODO: latest_at is just a happy path of latest_at_most_specific: they should never return
    // different results for a fully qualified descriptor anyhow.

    // TODO: I guess in practice this becomes Chunk::latest_at_most_specific_by_component_name
    // TODO: or rather, this now takes a descriptor...
    //
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
    pub fn latest_at(&self, query: &LatestAtQuery, component_desc: &ComponentDescriptor) -> Self {
        if self.is_empty() {
            return self.clone();
        }

        re_tracing::profile_function!(format!("{component_desc} @ {query:?}"));

        component_desc
            .fallbacks(true /* include_self */)
            .find_map(|desc| {
                let chunk = self.latest_at_exact_match(query, &desc);
                (!chunk.is_empty()).then_some(chunk)
            })
            .unwrap_or_else(|| self.emptied())
    }

    // TODO
    pub fn latest_at_exact_match(
        &self,
        query: &LatestAtQuery,
        component_desc: &ComponentDescriptor,
    ) -> Self {
        if self.is_empty() {
            return self.clone();
        }

        let Some(component_list_array) = self.components.get_by_descriptor(component_desc) else {
            return self.emptied();
        };

        self._latest_at(query, component_list_array)
    }

    #[cfg(TODO)]
    // TODO: there's never any reason to call this directly, is there? should this be private?
    fn latest_at_most_specific(
        &self,
        query: &LatestAtQuery,
        component_desc: &ComponentDescriptor,
    ) -> Self {
        // First, run the query for all matching descriptors.
        let mut results: IntMap<ComponentDescriptor, ((TimeInt, RowId), Self)> = self
            .components
            .get(&component_desc.component_name)
            .into_iter()
            .flat_map(|per_desc| per_desc.iter())
            .filter(|(desc, _list_array)| component_desc.encompasses(desc))
            .filter_map(|(desc, list_array)| {
                let chunk = self._latest_at(query, list_array);
                // NOTE: It's the only row and the only component anyhow.
                let index = chunk.iter_indices(&query.timeline()).next();
                index.map(|index| (desc.clone(), (index, chunk)))
            })
            .collect();

        if results.is_empty() {
            return self.clone();
        }

        if results.len() == 1 {
            return results
                .into_values()
                .next()
                .map(|(_index, chunk)| chunk)
                .unwrap_or_else(|| self.clone());
        }

        // TODO: at this point the results already only contain stuff that matches, so it's only a
        // matter of most specific now.

        // Then, only keep whichever result has the closest index to the queried one.
        //
        // There might be multiple, if we're very unlucky (same component, multiple tags, single row).
        //
        // Reminder: static always wins.
        let Some(max_index) = results.values().map(|(index, _chunk)| *index).max() else {
            return self.clone();
        };
        // TODO: make sure static wins (test it, I guess). also check what re_query does...
        results.retain(|_desc, (time, _chunk)| *time == max_index);

        if results.is_empty() {
            return self.clone();
        }

        if results.len() == 1 {
            return results
                .into_values()
                .next()
                .map(|(_index, chunk)| chunk)
                .unwrap_or_else(|| self.clone());
        }

        // Finally, we must keep whichever result has the most specific descriptor.
        //
        // There might be multiple, if we're extremely unlucky (same component, multiple tags, single row).
        let result = {
            let a = || {
                results.iter().find_map(|(desc, (_index, chunk))| {
                    (desc.archetype_name.is_some() && desc.archetype_field_name.is_some())
                        .then(|| chunk.clone())
                })
            };
            let b = || {
                results.iter().find_map(|(desc, (_index, chunk))| {
                    desc.archetype_name.is_some().then(|| chunk.clone())
                })
            };
            let c = || {
                results.iter().find_map(|(desc, (_index, chunk))| {
                    desc.archetype_field_name.is_some().then(|| chunk.clone())
                })
            };
            let d = || results.values().map(|(_index, chunk)| chunk.clone()).next();

            a().or_else(b).or_else(c).or_else(d)
        };

        result.unwrap_or_else(|| self.clone())
    }

    // TODO
    fn _latest_at(
        &self,
        query: &LatestAtQuery,
        component_list_array: &Arrow2ListArray<i32>,
    ) -> Self {
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
            let Some(time_column) = self.timelines.get(&query.timeline()) else {
                return self.emptied();
            };

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

        index.map_or_else(|| self.emptied(), |i| self.row_sliced(i, 1))
    }
}
