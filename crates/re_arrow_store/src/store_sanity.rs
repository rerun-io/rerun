use std::collections::BTreeMap;

use anyhow::{anyhow, ensure};
use nohash_hasher::IntMap;
use re_log_types::{TimeInt, Timeline};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexTable,
    PersistentComponentTable, PersistentIndexTable,
};

// TODO(#527): Typed errors.

// --- Data store ---

impl DataStore {
    /// Runs the sanity check suite for the entire datastore.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        // Row indices should be continuous across all index tables.
        if self.gc_id == 0 {
            let mut row_indices: IntMap<_, Vec<u64>> = IntMap::default();
            for table in self.indices.values() {
                for bucket in table.buckets.values() {
                    for (comp, index) in &bucket.indices.read().indices {
                        let row_indices = row_indices.entry(*comp).or_default();
                        row_indices.extend(index.iter().flatten().map(|row_idx| row_idx.as_u64()));
                    }
                }
            }

            for (comp, mut row_indices) in row_indices {
                // Not an actual row index!
                if comp == DataStore::insert_id_key() {
                    continue;
                }

                row_indices.sort();
                row_indices.dedup();
                for pair in row_indices.windows(2) {
                    let &[i1, i2] = pair else { unreachable!() };
                    ensure!(
                        i1 + 1 == i2,
                        "found hole in index coverage for {comp:?}: \
                            in {row_indices:?}, {i1} -> {i2}"
                    );
                }
            }
        }

        // Row indices should be continuous across all timeless index tables.
        {
            let mut row_indices: IntMap<_, Vec<u64>> = IntMap::default();
            for table in self.timeless_indices.values() {
                for (comp, index) in &table.indices {
                    let row_indices = row_indices.entry(*comp).or_default();
                    row_indices.extend(index.iter().flatten().map(|row_idx| row_idx.as_u64()));
                }
            }

            for (comp, mut row_indices) in row_indices {
                // Not an actual row index!
                if comp == DataStore::insert_id_key() {
                    continue;
                }

                row_indices.sort();
                row_indices.dedup();
                for pair in row_indices.windows(2) {
                    let &[i1, i2] = pair else { unreachable!() };
                    ensure!(
                        i1 + 1 == i2,
                        "found hole in timeless index coverage for {comp:?}: \
                            in {row_indices:?}, {i1} -> {i2}"
                    );
                }
            }
        }

        for table in self.timeless_indices.values() {
            table.sanity_check()?;
        }
        for table in self.timeless_components.values() {
            table.sanity_check()?;
        }

        for table in self.indices.values() {
            table.sanity_check()?;
        }
        for table in self.components.values() {
            table.sanity_check()?;
        }

        Ok(())
    }

    /// The oldest time for which we have any data.
    ///
    /// Ignores timeless data.
    ///
    /// Useful to call after a gc.
    pub fn oldest_time_per_timeline(&self) -> BTreeMap<Timeline, TimeInt> {
        crate::profile_function!();

        let mut oldest_time_per_timeline = BTreeMap::default();

        for component_table in self.components.values() {
            for bucket in &component_table.buckets {
                for (timeline, time_range) in &bucket.time_ranges {
                    let entry = oldest_time_per_timeline
                        .entry(*timeline)
                        .or_insert(TimeInt::MAX);
                    *entry = time_range.min.min(*entry);
                }
            }
        }

        oldest_time_per_timeline
    }
}

// --- Persistent Indices ---

impl PersistentIndexTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        let Self {
            ent_path: _,
            cluster_key,
            num_rows,
            indices,
            all_components: _,
        } = self;

        // All indices should be `Self::num_rows` long.
        {
            for (comp, index) in indices {
                let secondary_len = index.len() as u64;
                ensure!(
                    *num_rows == secondary_len,
                    "found rogue secondary index for {comp:?}: \
                        expected {num_rows} rows, got {secondary_len} instead",
                );
            }
        }

        // The cluster index must be fully dense.
        {
            let cluster_idx = indices
                .get(cluster_key)
                .ok_or_else(|| anyhow!("no index found for cluster key: {cluster_key:?}"))?;
            ensure!(
                cluster_idx.iter().all(|row| row.is_some()),
                "the cluster index ({cluster_key:?}) must be fully dense: \
                    got {cluster_idx:?}",
            );
        }

        Ok(())
    }
}

// --- Indices ---

impl IndexTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        // No two buckets should ever overlap time-range-wise.
        {
            let time_ranges = self
                .buckets
                .values()
                .map(|bucket| bucket.indices.read().time_range)
                .collect::<Vec<_>>();
            for time_ranges in time_ranges.windows(2) {
                let &[t1, t2] = time_ranges else { unreachable!() };
                ensure!(
                    t1.max.as_i64() < t2.min.as_i64(),
                    "found overlapping index buckets: {} ({}) <-> {} ({})",
                    self.timeline.typ().format(t1.max),
                    t1.max.as_i64(),
                    self.timeline.typ().format(t2.min),
                    t2.min.as_i64(),
                );
            }
        }

        // Run individual bucket sanity check suites too.
        for bucket in self.buckets.values() {
            bucket.sanity_check()?;
        }

        Ok(())
    }
}

impl IndexBucket {
    /// Runs the sanity check suite for the entire bucket.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        // All indices should contain the exact same number of rows as the time index.
        {
            let primary_len = times.len();
            for (comp, index) in indices {
                let secondary_len = index.len();
                ensure!(
                    primary_len == secondary_len,
                    "found rogue secondary index for {comp:?}: \
                        expected {primary_len} rows, got {secondary_len} instead",
                );
            }
        }

        // The cluster index must be fully dense.
        {
            let cluster_key = self.cluster_key;
            let cluster_idx = indices
                .get(&cluster_key)
                .ok_or_else(|| anyhow!("no index found for cluster key: {cluster_key:?}"))?;
            ensure!(
                cluster_idx.iter().all(|row| row.is_some()),
                "the cluster index ({cluster_key:?}) must be fully dense: \
                    got {cluster_idx:?}",
            );
        }

        Ok(())
    }
}

// --- Persistent Components ---

impl PersistentComponentTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        // All chunks should always be dense
        {
            for chunk in &self.chunks {
                ensure!(
                    chunk.validity().is_none(),
                    "persistent component chunks should always be dense",
                );
            }
        }

        Ok(())
    }
}

// --- Components ---

impl ComponentTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        // No two buckets should ever overlap row-range-wise.
        {
            let row_ranges = self
                .buckets
                .iter()
                .map(|bucket| bucket.row_offset..bucket.row_offset + bucket.total_rows())
                .collect::<Vec<_>>();
            for row_ranges in row_ranges.windows(2) {
                let &[r1, r2] = &row_ranges else { unreachable!() };
                ensure!(
                    !r1.contains(&r2.start),
                    "found overlapping component buckets: {r1:?} <-> {r2:?}"
                );
            }
        }

        for bucket in &self.buckets {
            bucket.sanity_check()?;
        }

        Ok(())
    }
}

impl ComponentBucket {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        crate::profile_function!();

        // All chunks should always be dense
        {
            for chunk in &self.chunks {
                ensure!(
                    chunk.validity().is_none(),
                    "component bucket chunks should always be dense",
                );
            }
        }

        Ok(())
    }
}
