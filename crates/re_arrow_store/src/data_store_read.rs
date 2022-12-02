use std::collections::HashMap;

use arrow2::array::{Array, MutableArray, UInt64Vec};
use polars::prelude::{DataFrame, Series};

use re_log_types::{ObjPath as EntityPath, Timeline};

use crate::{
    ComponentBucket, ComponentNameRef, ComponentTable, DataStore, IndexBucket, IndexTable, RowIndex,
};

// ---

/// A query in time.
#[derive(Clone, Debug)]
pub enum TimeQuery {
    /// Get the latest version of the data available at this time.
    LatestAt(i64),

    /// Get all the data within this time interval, plus the latest
    /// one before the start of the interval.
    ///
    /// Motivation: all data is considered alive untl the next logging
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
        let latest_at = match time_query {
            TimeQuery::LatestAt(latest_at) => *latest_at,
            #[allow(clippy::todo)]
            TimeQuery::Range(_) => todo!("implement range queries!"),
        };

        let row_indices = self
            .indices
            .get_mut(&(*timeline, ent_path.clone()))
            .map(|index| index.latest_at(latest_at, components))
            .unwrap();

        let mut series: HashMap<_, _> = row_indices
            .into_iter()
            .filter_map(|(name, row_idx)| {
                self.components
                    .get(name)
                    .map(|table| (name, Series::try_from((name, table.get(row_idx))).unwrap()))
            })
            .collect();

        let series_ordered = components
            .iter()
            .filter_map(|name| series.remove(name))
            .collect();
        DataFrame::new(series_ordered).map_err(Into::into)
    }
}

// --- Indices ---

impl IndexTable {
    pub fn latest_at<'a>(
        &mut self,
        at: i64,
        components: &[ComponentNameRef<'a>],
    ) -> HashMap<ComponentNameRef<'a>, RowIndex> {
        let bucket = self.buckets.iter_mut().next().unwrap().1;
        bucket.latest_at(at, components)
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
        };

        // Yep, the reshuffle implentation is very dumb and very slow :)
        // TODO(#442): re_datastore: implement efficient shuffling on the read path.

        // shuffle time index back into a sorted state
        {
            // The time index must always be dense, thus it shouldn't even have a validity
            // bitmap attached to it to begin with.
            assert!(self.times.validity().is_none());

            let source = self.times.values().clone();
            let values = self.times.values_mut_slice();

            for (from, to) in swaps.iter().enumerate() {
                values[*to] = source[from];
            }
        }

        fn reshuffle_index(index: &mut UInt64Vec, swaps: &[usize]) {
            // shuffle data
            {
                let source = index.values().clone();
                let values = index.values_mut_slice();

                for (from, to) in swaps.iter().enumerate() {
                    values[*to] = source[from];
                }
            }

            // shuffle validity bitmaps
            let validity_before = index.validity().cloned();
            let validity_after = validity_before.clone();
            if let (Some(validity_before), Some(mut validity_after)) =
                (validity_before, validity_after)
            {
                for (from, to) in swaps.iter().enumerate() {
                    validity_after.set(*to, validity_before.get(from));
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

    pub fn latest_at<'a>(
        &mut self,
        at: i64,
        components: &[ComponentNameRef<'a>],
    ) -> HashMap<ComponentNameRef<'a>, RowIndex> {
        self.sort_indices();

        // find the corresponding row index within the time index
        let times = self.times.values();
        let time_row_idx = match times.binary_search(&at) {
            Ok(time_row_idx) => time_row_idx as i64,
            Err(time_row_idx_closest) => time_row_idx_closest.clamp(0, times.len() - 1) as i64,
        };

        components
            .iter()
            .filter_map(|name| self.indices.get(*name).map(|index| (name, index)))
            .filter_map(|(name, index)| {
                let mut row_idx = time_row_idx;
                while !index.is_valid(row_idx as _) {
                    row_idx -= 1;
                    if row_idx < 0 {
                        return None;
                    }
                }

                assert!(index.is_valid(row_idx as usize));
                (*name, index.values()[row_idx as usize]).into()
            })
            .collect()
    }
}

// --- Components ---

impl ComponentTable {
    pub fn get(&self, row_idx: u64) -> Box<dyn Array> {
        let bucket = &self.buckets[&0];
        bucket.get(row_idx)
    }
}

impl ComponentBucket {
    pub fn get(&self, row_idx: u64) -> Box<dyn Array> {
        let row_idx = row_idx - self.row_offset;
        self.data.slice(row_idx as usize, 1)
    }
}
