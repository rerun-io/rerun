use std::collections::HashMap;

use arrow2::array::{Array, MutableArray, UInt64Vec};
use polars::prelude::{DataFrame, Series};

use re_log::debug;
use re_log_types::{ComponentNameRef, ObjPath as EntityPath, TimeInt, Timeline};

use crate::{ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexTable, RowIndex};

// ---

/// A query in time.
#[derive(Clone, Debug)]
pub enum TimeQuery {
    /// Get the latest version of the data available at this time.
    LatestAt(i64),

    /// Get all the data within this time interval, plus the latest
    /// one before the start of the interval.
    ///
    /// Motivation: all data is considered alive until the next logging
    /// to the same data path.
    Range(std::ops::RangeInclusive<i64>),
}

impl TimeQuery {
    pub const EVERYTHING: Self = Self::Range(i64::MIN..=i64::MAX);
}

// --- Data store ---

impl DataStore {
    pub fn query(
        &mut self,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>],
    ) -> anyhow::Result<DataFrame> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id += 1;

        let ent_path_hash = ent_path.hash();

        let latest_at = match time_query {
            TimeQuery::LatestAt(latest_at) => *latest_at,
            #[allow(clippy::todo)]
            TimeQuery::Range(_) => todo!("implement range queries!"),
        };

        debug!(
            kind = "query",
            id = self.query_id,
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            ?components,
            "query started..."
        );

        let row_indices = self
            .indices
            .get_mut(&(*timeline, *ent_path_hash))
            .map(|index| index.latest_at(latest_at, components))
            .unwrap_or_default();
        debug!(
            kind = "query",
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            ?components,
            ?row_indices,
            "row indices fetched"
        );

        let mut series: HashMap<_, _> = row_indices
            .into_iter()
            .filter_map(|(name, row_idx)| {
                self.components.get(name).and_then(|table| {
                    table
                        .get(row_idx)
                        .map(|data| (name, Series::try_from((name, data)).unwrap()))
                })
            })
            .collect();

        let series_ordered = components
            .iter()
            .filter_map(|name| series.remove(name))
            .collect();
        let df = DataFrame::new(series_ordered)?;

        df.explode(df.get_column_names()).map_err(Into::into)
    }

    /// Force the sorting of all indices.
    pub fn sort_indices(&mut self) {
        for index in self.indices.values_mut() {
            index.sort_indices();
        }
    }
}

// --- Indices ---

impl IndexTable {
    pub fn latest_at<'a>(
        &mut self,
        at: i64,
        components: &[ComponentNameRef<'a>],
    ) -> HashMap<ComponentNameRef<'a>, RowIndex> {
        let mut results = HashMap::with_capacity(components.len());

        let timeline = self.timeline;
        for &name in components {
            'for_each_bucket: for (i, bucket) in self.iter_bucket_mut(at).enumerate() {
                debug!(
                    kind = "query",
                    component = name,
                    timeline = %timeline.name(),
                    time = timeline.typ().format(at.into()),
                    attempt = i,
                    time_range = ?bucket.time_range.min.as_i64()..=bucket.time_range.max.as_i64(),
                    "found candidate bucket"
                );
                if let Some(row_idx) = bucket.latest_at(at, name) {
                    results.insert(name, row_idx);
                    break 'for_each_bucket; // better safe than sorry
                }
            }
        }

        results
    }

    // TODO: doc
    pub fn find_bucket_mut(&mut self, at: i64) -> &mut IndexBucket {
        // TODO: explain why this cannot fail
        self.iter_bucket_mut(at).next().unwrap()
    }

    // TODO: doc
    pub fn iter_bucket_mut(&mut self, at: i64) -> impl Iterator<Item = &mut IndexBucket> {
        self.buckets
            .range_mut(..=TimeInt::from(at))
            .rev()
            .map(|(_, bucket)| bucket)
    }

    /// Force the sorting of all buckets.
    pub fn sort_indices(&mut self) {
        for bucket in self.buckets.values_mut() {
            bucket.sort_indices();
        }
    }
}

impl IndexBucket {
    /// Sort all indices by time.
    pub fn sort_indices(&mut self) {
        if self.is_sorted {
            return;
        }

        let swaps = {
            let times = self.times.values();
            let mut swaps = (0..times.len()).collect::<Vec<_>>();
            swaps.sort_by_key(|&i| &times[i]);
            swaps
                .iter()
                .copied()
                .enumerate()
                .map(|(to, from)| (from, to))
                .collect::<Vec<_>>()
        };

        // Yep, the reshuffle implementation is very dumb and very slow :)
        // TODO(#442): re_datastore: implement efficient shuffling on the read path.

        // shuffle time index back into a sorted state
        {
            // The time index must always be dense, thus it shouldn't even have a validity
            // bitmap attached to it to begin with.
            assert!(self.times.validity().is_none());

            let source = self.times.values().clone();
            let values = self.times.values_mut_slice();

            for (from, to) in swaps.iter().copied() {
                values[to] = source[from];
            }
        }

        fn reshuffle_index(index: &mut UInt64Vec, swaps: &[(usize, usize)]) {
            // shuffle data
            {
                let source = index.values().clone();
                let values = index.values_mut_slice();

                for (from, to) in swaps.iter().copied() {
                    values[to] = source[from];
                }
            }

            // shuffle validity bitmaps
            let validity_before = index.validity().cloned();
            let validity_after = validity_before.clone();
            if let (Some(validity_before), Some(mut validity_after)) =
                (validity_before, validity_after)
            {
                for (from, to) in swaps.iter().copied() {
                    validity_after.set(to, validity_before.get(from));
                }

                // we expect as many nulls before and after.
                assert_eq!(validity_before.unset_bits(), validity_after.unset_bits());

                index.set_validity(Some(validity_after));
            }
        }

        // shuffle component indices back into a sorted state
        for index in self.indices.values_mut() {
            reshuffle_index(index, &swaps);
        }

        self.is_sorted = true;
    }

    pub fn latest_at<'a>(&mut self, at: i64, component: ComponentNameRef<'a>) -> Option<RowIndex> {
        self.sort_indices();

        debug!(
            kind = "query",
            component,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(at.into()),
            "searching for primary & secondary row indices..."
        );

        // find the primary index's row.
        let times = self.times.values();
        let primary_idx = times.partition_point(|time| *time <= at) as i64;

        // The partition point is always _beyond_ the index that we're looking for.
        // A partition point of 0 thus means that we're trying to query for data that lives
        // _before_ the beginning of time... there's nothing to be found there.
        if primary_idx == 0 {
            return None;
        }

        // The partition point is always _beyond_ the index that we're looking for; we need
        // to step back to find what we came for.
        let primary_idx = primary_idx - 1;
        debug!(
            kind = "query",
            component,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(at.into()),
            %primary_idx,
            "found primary index",
        );

        // find the secondary indices' rows, and the associated row indices.
        self.indices
            .get_key_value(component)
            .and_then(|(name, index)| {
                let mut secondary_idx = primary_idx;
                while !index.is_valid(secondary_idx as _) {
                    secondary_idx -= 1;
                    if secondary_idx < 0 {
                        debug!(
                            kind = "query",
                            component = name,
                            timeline = %self.timeline.name(),
                            time = self.timeline.typ().format(at.into()),
                            %primary_idx,
                            "no secondary index found",
                        );
                        return None;
                    }
                }

                assert!(index.is_valid(secondary_idx as usize));
                let row_idx = index.values()[secondary_idx as usize];

                debug!(
                    kind = "query",
                    component = name,
                    timeline = %self.timeline.name(),
                    time = self.timeline.typ().format(at.into()),
                    %primary_idx, %secondary_idx, %row_idx,
                    "found secondary index + row index",
                );

                Some(row_idx)
            })
    }
}

// --- Components ---

impl ComponentTable {
    pub fn get(&self, row_idx: RowIndex) -> Option<Box<dyn Array>> {
        let mut bucket_nr = self
            .buckets
            .partition_point(|bucket| row_idx >= bucket.row_offset);

        // The partition point will give us the index of the first bucket that has a row offset
        // strictly greater than the row index we're looking for, therefore we need to take a
        // step back to find what we're looking for.
        //
        // Since component tables always spawn with a default bucket at offset 0, the smallest
        // partition point that can ever be returned is one, thus this operation is overflow-safe.
        debug_assert!(bucket_nr > 0);
        bucket_nr -= 1;

        if let Some(bucket) = self.buckets.get(bucket_nr) {
            debug!(
                kind = "query",
                component = self.name.as_str(),
                row_idx,
                bucket_nr,
                bucket.row_offset,
                "fetching component data"
            );
            Some(bucket.get(row_idx))
        } else {
            debug!(
                kind = "query",
                component = self.name.as_str(),
                row_idx,
                bucket_nr,
                "row index is out of bounds"
            );
            None
        }
    }
}
impl ComponentBucket {
    // Panics on out-of-bounds
    pub fn get(&self, row_idx: u64) -> Box<dyn Array> {
        let row_idx = row_idx - self.row_offset;
        self.data.slice(row_idx as usize, 1)
    }
}
