use std::collections::BTreeMap;

use ahash::{HashMap, HashSet};

use nohash_hasher::IntMap;
use re_log_types::{
    EntityPath, EntityPathHash, RowId, TimeInt, TimePoint, TimeRange, Timeline,
    VecDequeRemovalExt as _,
};
use re_types_core::{ComponentName, SizeBytes as _};

use crate::{
    store::{
        ClusterCellCache, IndexedBucket, IndexedBucketInner, IndexedTable, PersistentIndexedTable,
        PersistentIndexedTableInner,
    },
    DataStore, DataStoreStats, StoreDiff, StoreDiffKind, StoreEvent,
};

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given fraction.
    ///
    /// The fraction must be a float in the range [0.0 : 1.0].
    DropAtLeastFraction(f64),

    /// GC Everything that isn't protected
    Everything,
}

#[derive(Debug, Clone)]
pub struct GarbageCollectionOptions {
    /// What target threshold should the GC try to meet.
    pub target: GarbageCollectionTarget,

    /// Whether to also GC timeless data.
    pub gc_timeless: bool,

    /// How many component revisions to preserve on each timeline.
    pub protect_latest: usize,

    /// Whether to purge tables that no longer contain any data
    pub purge_empty_tables: bool,

    /// Components which should not be protected from GC when using `protect_latest`
    pub dont_protect: HashSet<ComponentName>,

    /// Whether to enable batched bucket drops.
    ///
    /// Disabled by default as it is currently slower in most cases (somehow).
    pub enable_batching: bool,
}

impl GarbageCollectionOptions {
    pub fn gc_everything() -> Self {
        GarbageCollectionOptions {
            target: GarbageCollectionTarget::Everything,
            gc_timeless: true,
            protect_latest: 0,
            purge_empty_tables: true,
            dont_protect: Default::default(),
            enable_batching: false,
        }
    }
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                write!(f, "DropAtLeast({:.3}%)", re_format::format_f64(*p * 100.0))
            }
            GarbageCollectionTarget::Everything => write!(f, "Everything"),
        }
    }
}

impl DataStore {
    /// Triggers a garbage collection according to the desired `target`.
    ///
    /// Garbage collection's performance is bounded by the number of buckets in each table (for
    /// each `RowId`, we have to find the corresponding bucket, which is roughly `O(log(n))`) as
    /// well as the number of rows in each of those buckets (for each `RowId`, we have to sort the
    /// corresponding bucket (roughly `O(n*log(n))`) and then find the corresponding row (roughly
    /// `O(log(n))`.
    /// The size of the data itself has no impact on performance.
    ///
    /// Returns the list of `RowId`s that were purged from the store.
    ///
    /// ## Semantics
    ///
    /// Garbage collection works on a row-level basis and is driven by [`RowId`] order,
    /// i.e. the order defined by the clients' wall-clocks, allowing it to drop data across
    /// the different timelines in a fair, deterministic manner.
    /// Similarly, out-of-order data is supported out of the box.
    ///
    /// The garbage collector doesn't deallocate data in and of itself: all it does is drop the
    /// store's internal references to that data (the `DataCell`s), which will be deallocated once
    /// their reference count reaches 0.
    ///
    /// ## Limitations
    ///
    /// The garbage collector has limited support for latest-at semantics. The configuration option:
    /// [`GarbageCollectionOptions::protect_latest`] will protect the N latest values of each
    /// component on each timeline. The only practical guarantee this gives is that a latest-at query
    /// with a value of max-int will be unchanged. However, latest-at queries from other arbitrary
    /// points in time may provide different results pre- and post- GC.
    pub fn gc(&mut self, options: &GarbageCollectionOptions) -> (Vec<StoreEvent>, DataStoreStats) {
        re_tracing::profile_function!();

        self.gc_id += 1;

        let stats_before = DataStoreStats::from_store(self);

        let (initial_num_rows, initial_num_bytes) =
            stats_before.total_rows_and_bytes_with_timeless(options.gc_timeless);

        let protected_rows =
            self.find_all_protected_rows(options.protect_latest, &options.dont_protect);

        let mut diffs = match options.target {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                assert!((0.0..=1.0).contains(&p));

                let num_bytes_to_drop = initial_num_bytes * p;
                let target_num_bytes = initial_num_bytes - num_bytes_to_drop;

                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    initial_num_rows = re_format::format_number(initial_num_rows as _),
                    initial_num_bytes = re_format::format_bytes(initial_num_bytes),
                    target_num_bytes = re_format::format_bytes(target_num_bytes),
                    drop_at_least_num_bytes = re_format::format_bytes(num_bytes_to_drop),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(
                    options.enable_batching,
                    num_bytes_to_drop,
                    options.gc_timeless,
                    &protected_rows,
                )
            }
            GarbageCollectionTarget::Everything => {
                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    initial_num_rows = re_format::format_number(initial_num_rows as _),
                    initial_num_bytes = re_format::format_bytes(initial_num_bytes),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(
                    options.enable_batching,
                    f64::INFINITY,
                    options.gc_timeless,
                    &protected_rows,
                )
            }
        };

        if options.purge_empty_tables {
            diffs.extend(self.purge_empty_tables());
        }

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        // NOTE: only temporal data and row metadata get purged!
        let stats_after = DataStoreStats::from_store(self);
        let (new_num_rows, new_num_bytes) =
            stats_after.total_rows_and_bytes_with_timeless(options.gc_timeless);

        re_log::trace!(
            kind = "gc",
            id = self.gc_id,
            %options.target,
            initial_num_rows = re_format::format_number(initial_num_rows as _),
            initial_num_bytes = re_format::format_bytes(initial_num_bytes),
            new_num_rows = re_format::format_number(new_num_rows as _),
            new_num_bytes = re_format::format_bytes(new_num_bytes),
            "GC done"
        );

        let stats_diff = stats_before - stats_after;

        let events: Vec<_> = diffs
            .into_iter()
            .map(|diff| StoreEvent {
                store_id: self.id.clone(),
                store_generation: self.generation(),
                event_id: self
                    .event_id
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                diff,
            })
            .collect();

        {
            if cfg!(debug_assertions) {
                let any_event_other_than_deletion =
                    events.iter().any(|e| e.kind != StoreDiffKind::Deletion);
                assert!(!any_event_other_than_deletion);
            }

            Self::on_events(&events);
        }

        (events, stats_diff)
    }

    /// Tries to drop _at least_ `num_bytes_to_drop` bytes of data from the store.
    ///
    /// Returns the list of `RowId`s that were purged from the store.
    fn gc_drop_at_least_num_bytes(
        &mut self,
        enable_batching: bool,
        mut num_bytes_to_drop: f64,
        include_timeless: bool,
        protected_rows: &HashSet<RowId>,
    ) -> Vec<StoreDiff> {
        re_tracing::profile_function!();

        let mut diffs = Vec::new();

        // The algorithm is straightforward:
        // 1. Find the oldest `RowId` that is not protected
        // 2. Find all tables that potentially hold data associated with that `RowId`
        // 3. Drop the associated row and account for the space we got back

        let batch_size = (self.config.indexed_bucket_num_rows as usize).saturating_mul(2);
        let batch_size = batch_size.clamp(64, 4096);
        // let batch_size = 1;
        let mut batch: Vec<(TimePoint, (EntityPathHash, RowId))> = Vec::with_capacity(batch_size);
        let mut batch_is_protected = false;

        let Self {
            cluster_key,
            metadata_registry,
            cluster_cell_cache,
            tables,
            timeless_tables,
            ..
        } = self;

        for (&row_id, (timepoint, entity_path_hash)) in &metadata_registry.registry {
            if protected_rows.contains(&row_id) {
                batch_is_protected = true;
                continue;
            }

            batch.push((timepoint.clone(), (*entity_path_hash, row_id)));
            if batch.len() < batch_size {
                continue;
            }

            let dropped = Self::drop_batch(
                enable_batching,
                tables,
                timeless_tables,
                cluster_cell_cache,
                *cluster_key,
                include_timeless,
                &mut num_bytes_to_drop,
                &batch,
                batch_is_protected,
            );

            // Only decrement the metadata size trackers if we're actually certain that we'll drop
            // that RowId in the end.
            for dropped in dropped {
                let metadata_dropped_size_bytes = dropped.row_id.total_size_bytes()
                    + dropped.timepoint().total_size_bytes()
                    + dropped.entity_path.hash().total_size_bytes();
                metadata_registry.heap_size_bytes = metadata_registry
                    .heap_size_bytes
                    .checked_sub(metadata_dropped_size_bytes)
                    .unwrap_or_else(|| {
                        re_log::warn_once!(
                            "GC metadata_registry size tracker underflowed, this is a bug!"
                        );
                        0
                    });
                num_bytes_to_drop -= metadata_dropped_size_bytes as f64;

                diffs.push(dropped);
            }

            if num_bytes_to_drop <= 0.0 {
                break;
            }

            batch.clear();
            batch_is_protected = false;
        }

        // Handle leftovers.
        {
            let dropped = Self::drop_batch(
                enable_batching,
                tables,
                timeless_tables,
                cluster_cell_cache,
                *cluster_key,
                include_timeless,
                &mut num_bytes_to_drop,
                &batch,
                batch_is_protected,
            );

            // Only decrement the metadata size trackers if we're actually certain that we'll drop
            // that RowId in the end.
            for dropped in dropped {
                let metadata_dropped_size_bytes = dropped.row_id.total_size_bytes()
                    + dropped.timepoint().total_size_bytes()
                    + dropped.entity_path.hash().total_size_bytes();
                metadata_registry.heap_size_bytes = metadata_registry
                    .heap_size_bytes
                    .checked_sub(metadata_dropped_size_bytes)
                    .unwrap_or_else(|| {
                        re_log::warn_once!(
                            "GC metadata_registry size tracker underflowed, this is a bug!"
                        );
                        0
                    });
                num_bytes_to_drop -= metadata_dropped_size_bytes as f64;

                diffs.push(dropped);
            }
        }

        // Purge the removed rows from the metadata_registry.
        // This is safe because the entire GC process is driven by RowId-order.
        for diff in &diffs {
            metadata_registry.remove(&diff.row_id);
        }

        diffs
    }

    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    fn drop_batch(
        enable_batching: bool,
        tables: &mut BTreeMap<(EntityPathHash, Timeline), IndexedTable>,
        timeless_tables: &mut IntMap<EntityPathHash, PersistentIndexedTable>,
        cluster_cell_cache: &ClusterCellCache,
        cluster_key: ComponentName,
        include_timeless: bool,
        num_bytes_to_drop: &mut f64,
        batch: &[(TimePoint, (EntityPathHash, RowId))],
        batch_is_protected: bool,
    ) -> Vec<StoreDiff> {
        let mut diffs = Vec::new();

        // NOTE: The batch is already sorted by definition since it's extracted from the registry's btreemap.
        let max_row_id = batch.last().map(|(_, (_, row_id))| *row_id);

        if enable_batching && max_row_id.is_some() && !batch_is_protected {
            // NOTE: unwrap cannot fail but just a precaution in case this code moves around…
            let max_row_id = max_row_id.unwrap_or(RowId::ZERO);

            let mut batch_removed: HashMap<RowId, StoreDiff> = HashMap::default();
            let mut cur_entity_path_hash = None;

            // NOTE: We _must_  go through all tables no matter what, since the batch might contain
            // any number of distinct entities.
            for ((entity_path_hash, _), table) in &mut *tables {
                let (removed, num_bytes_removed) =
                    table.try_drop_bucket(cluster_cell_cache, cluster_key, max_row_id);

                *num_bytes_to_drop -= num_bytes_removed as f64;

                if cur_entity_path_hash != Some(*entity_path_hash) {
                    diffs.extend(batch_removed.drain().map(|(_, diff)| diff));

                    cur_entity_path_hash = Some(*entity_path_hash);
                }

                for mut removed in removed {
                    batch_removed
                        .entry(removed.row_id)
                        .and_modify(|diff| {
                            diff.times.extend(std::mem::take(&mut removed.times));
                        })
                        .or_insert(removed);
                }
            }

            diffs.extend(batch_removed.drain().map(|(_, diff)| diff));
        }

        if *num_bytes_to_drop <= 0.0 {
            return diffs;
        }

        for (timepoint, (entity_path_hash, row_id)) in batch {
            let mut diff: Option<StoreDiff> = None;

            // find all tables that could possibly contain this `RowId`
            for (&timeline, &time) in timepoint {
                if let Some(table) = tables.get_mut(&(*entity_path_hash, timeline)) {
                    let (removed, num_bytes_removed) =
                        table.try_drop_row(cluster_cell_cache, *row_id, time.as_i64());
                    if let Some(inner) = diff.as_mut() {
                        if let Some(removed) = removed {
                            inner.times.extend(removed.times);
                        }
                    } else {
                        diff = removed;
                    }
                    *num_bytes_to_drop -= num_bytes_removed as f64;
                }
            }

            // TODO(jleibs): This is a worst-case removal-order. Would be nice to collect all the rows
            // first and then remove them in one pass.
            if timepoint.is_timeless() && include_timeless {
                for table in timeless_tables.values_mut() {
                    // let deleted_comps = deleted.timeless.entry(ent_path.clone()_hash).or_default();
                    let (removed, num_bytes_removed) =
                        table.try_drop_row(cluster_cell_cache, *row_id);
                    if let Some(inner) = diff.as_mut() {
                        if let Some(removed) = removed {
                            inner.times.extend(removed.times);
                        }
                    } else {
                        diff = removed;
                    }
                    *num_bytes_to_drop -= num_bytes_removed as f64;
                }
            }

            diffs.extend(diff);

            if *num_bytes_to_drop <= 0.0 {
                break;
            }
        }

        diffs
    }

    /// For each `EntityPath`, `Timeline`, `Component` find the N latest [`RowId`]s.
    //
    // TODO(jleibs): More complex functionality might required expanding this to also
    // *ignore* specific entities, components, timelines, etc. for this protection.
    //
    // TODO(jleibs): `RowId`s should never overlap between entities. Creating a single large
    // HashSet might actually be sub-optimal here. Consider switching to a map of
    // `EntityPath` -> `HashSet<RowId>`.
    // Update: this is true-er than ever before now that RowIds are truly unique!
    fn find_all_protected_rows(
        &mut self,
        target_count: usize,
        dont_protect: &HashSet<ComponentName>,
    ) -> HashSet<RowId> {
        re_tracing::profile_function!();

        if target_count == 0 {
            return Default::default();
        }

        // We need to sort to be able to determine latest-at.
        self.sort_indices_if_needed();

        let mut protected_rows: HashSet<RowId> = Default::default();

        // Find all protected rows in regular indexed tables
        for table in self.tables.values() {
            let mut components_to_find: HashMap<ComponentName, usize> = table
                .all_components
                .iter()
                .filter(|c| **c != table.cluster_key)
                .filter(|c| !dont_protect.contains(*c))
                .map(|c| (*c, target_count))
                .collect();

            for bucket in table.buckets.values().rev() {
                for (component, count) in &mut components_to_find {
                    if *count == 0 {
                        continue;
                    }
                    let inner = bucket.inner.read();
                    // TODO(jleibs): If the entire column for a component is empty, we should
                    // make sure the column is dropped so we don't have to iterate over a
                    // bunch of Nones.
                    if let Some(column) = inner.columns.get(component) {
                        for row in column
                            .iter()
                            .enumerate()
                            .rev()
                            .filter_map(|(row_index, cell)| {
                                cell.as_ref().and_then(|_| inner.col_row_id.get(row_index))
                            })
                            .take(*count)
                        {
                            *count -= 1;
                            protected_rows.insert(*row);
                        }
                    }
                }
            }
        }

        // Find all protected rows in timeless tables
        for table in self.timeless_tables.values() {
            let cluster_key = table.cluster_key;
            let table = table.inner.read();
            let mut components_to_find: HashMap<ComponentName, usize> = table
                .columns
                .keys()
                .filter(|c| **c != cluster_key)
                .filter(|c| !dont_protect.contains(*c))
                .map(|c| (*c, target_count))
                .collect();

            for (component, count) in &mut components_to_find {
                if *count == 0 {
                    continue;
                }
                // TODO(jleibs): If the entire column for a component is empty, we should
                // make sure the column is dropped so we don't have to iterate over a
                // bunch of Nones.
                if let Some(column) = table.columns.get(component) {
                    for row_id in column
                        .iter()
                        .enumerate()
                        .rev()
                        .filter_map(|(row_index, cell)| {
                            cell.as_ref().and_then(|_| table.col_row_id.get(row_index))
                        })
                        .take(*count)
                    {
                        *count -= 1;
                        protected_rows.insert(*row_id);
                    }
                }
            }
        }

        protected_rows
    }

    /// Remove any tables which contain only components which are empty.
    // TODO(jleibs): We could optimize this further by also erasing empty columns.
    fn purge_empty_tables(&mut self) -> impl Iterator<Item = StoreDiff> {
        re_tracing::profile_function!();

        let mut diffs: BTreeMap<RowId, StoreDiff> = BTreeMap::default();

        // Drop any empty timeless tables
        self.timeless_tables.retain(|_, table| {
            let entity_path = &table.ent_path;
            let mut table = table.inner.write();

            // If any column is non-empty, we need to keep this table…
            for num in &table.col_num_instances {
                if num.get() != 0 {
                    return true;
                }
            }

            // …otherwise we can drop it.

            let entity_path = entity_path.clone();

            for i in 0..table.col_row_id.len() {
                let row_id = table.col_row_id[i];

                let mut diff = StoreDiff::deletion(row_id, entity_path.clone());

                for column in &mut table.columns.values_mut() {
                    let cell = column[i].take();
                    if let Some(cell) = cell {
                        diff.cells.insert(cell.component_name(), cell);
                    }
                }

                let previous_value = diffs.insert(row_id, diff);
                // Reminder: this is a timeless table, therefore this `RowId` and the data associated
                // with it cannot exist anywhere else.
                debug_assert!(previous_value.is_none());
            }

            false
        });

        // Drop any empty temporal tables that aren't backed by a timeless table
        self.tables.retain(|(entity, _), table| {
            // If the timeless table still exists, this table might be storing empty values
            // that hide the timeless values, so keep it around.
            if self.timeless_tables.contains_key(entity) {
                return true;
            }

            // If any bucket has a non-empty component in any column, we keep it…
            for bucket in table.buckets.values() {
                let inner = bucket.inner.read();
                for num in &inner.col_num_instances {
                    if num.get() != 0 {
                        return true;
                    }
                }
            }

            // …otherwise we can drop it.

            let entity_path = table.ent_path.clone();

            for bucket in table.buckets.values() {
                let mut inner = bucket.inner.write();

                for i in 0..inner.col_row_id.len() {
                    let row_id = inner.col_row_id[i];
                    let time = inner.col_time[i];

                    let diff = diffs
                        .entry(row_id)
                        .or_insert_with(|| StoreDiff::deletion(row_id, entity_path.clone()));

                    diff.times.push((bucket.timeline, time.into()));

                    for column in &mut inner.columns.values_mut() {
                        let cell = column[i].take();
                        if let Some(cell) = cell {
                            diff.cells.insert(cell.component_name(), cell);
                        }
                    }
                }
            }

            false
        });

        // TODO(cmc): Hmm, this is dropping buckets but doesn't seem to handle the case where all
        // buckets are removed (which is an illegal state).
        // Doesn't seem to handle the case where the only bucket left isn't indexed at -inf either.

        diffs.into_values()
    }
}

impl IndexedTable {
    /// Try to drop an entire bucket at once if it doesn't contain any `RowId` greater than `max_row_id`.
    fn try_drop_bucket(
        &mut self,
        cluster_cache: &ClusterCellCache,
        cluster_key: ComponentName,
        max_row_id: RowId,
    ) -> (Vec<StoreDiff>, u64) {
        re_tracing::profile_function!();

        let ent_path = self.ent_path.clone();
        let timeline = self.timeline;

        let mut diffs: Vec<StoreDiff> = Vec::new();
        let mut dropped_num_bytes = 0u64;
        let mut dropped_num_rows = 0u64;

        let mut dropped_bucket_times = HashSet::default();

        // TODO(cmc): scaling linearly with the number of buckets could be improved, although this
        // is quite fast in practice because of the early check.
        for (bucket_time, bucket) in &self.buckets {
            let inner = &mut *bucket.inner.write();

            if inner.col_time.is_empty() || max_row_id < inner.max_row_id {
                continue;
            }

            let IndexedBucketInner {
                mut col_time,
                mut col_row_id,
                mut columns,
                size_bytes,
                ..
            } = std::mem::take(inner);

            dropped_bucket_times.insert(*bucket_time);

            while let Some(row_id) = col_row_id.pop_front() {
                let mut diff = StoreDiff::deletion(row_id, ent_path.clone());

                if let Some(time) = col_time.pop_front() {
                    diff.times.push((timeline, time.into()));
                }

                for (component_name, column) in &mut columns {
                    if let Some(cell) = column.pop_front().flatten() {
                        if cell.component_name() == cluster_key {
                            if let Some(cached_cell) = cluster_cache.get(&cell.num_instances()) {
                                if std::ptr::eq(cell.as_ptr(), cached_cell.as_ptr()) {
                                    // We don't fire events when inserting autogenerated cluster cells, and
                                    // therefore must not fire when removing them either.
                                    continue;
                                }
                            }
                        }

                        diff.cells.insert(*component_name, cell);
                    }
                }

                diffs.push(diff);
            }

            dropped_num_bytes += size_bytes;
            dropped_num_rows += col_time.len() as u64;
        }

        self.buckets
            .retain(|bucket_time, _| !dropped_bucket_times.contains(bucket_time));

        if self.buckets.is_empty() {
            let Self {
                timeline,
                ent_path: _,
                cluster_key,
                buckets,
                all_components: _, // keep the history on purpose
                buckets_num_rows,
                buckets_size_bytes,
            } = self;

            let bucket = IndexedBucket::new(*cluster_key, *timeline);
            let size_bytes = bucket.total_size_bytes();

            *buckets = [(i64::MIN.into(), bucket)].into();
            *buckets_num_rows = 0;
            *buckets_size_bytes = size_bytes;

            return (diffs, dropped_num_bytes);
        }

        // NOTE: Make sure the first bucket is responsible for `-∞`, which might or might not be
        // the case now that we've been moving buckets around.
        if let Some((_, bucket)) = self.buckets.pop_first() {
            self.buckets.insert(TimeInt::MIN, bucket);
        }

        self.buckets_num_rows -= dropped_num_rows;
        self.buckets_size_bytes -= dropped_num_bytes;

        (diffs, dropped_num_bytes)
    }

    /// Tries to drop the given `row_id` from the table, which is expected to be found at the
    /// specified `time`.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(
        &mut self,
        cluster_cache: &ClusterCellCache,
        row_id: RowId,
        time: i64,
    ) -> (Option<StoreDiff>, u64) {
        re_tracing::profile_function!();

        let ent_path = self.ent_path.clone();
        let timeline = self.timeline;
        let cluster_key = self.cluster_key;

        let table_has_more_than_one_bucket = self.buckets.len() > 1;

        let (bucket_key, bucket) = self.find_bucket_mut(time.into());
        let bucket_num_bytes = bucket.total_size_bytes();

        let (diff, mut dropped_num_bytes) = {
            let inner = &mut *bucket.inner.write();
            inner.try_drop_row(
                cluster_cache,
                cluster_key,
                row_id,
                timeline,
                &ent_path,
                time,
            )
        };

        // NOTE: We always need to keep at least one bucket alive, otherwise we have
        // nowhere to write to.
        if table_has_more_than_one_bucket && bucket.num_rows() == 0 {
            // NOTE: We're dropping the bucket itself in this case, rather than just its
            // contents.
            debug_assert!(
                dropped_num_bytes <= bucket_num_bytes,
                "Bucket contained more bytes than it thought"
            );
            dropped_num_bytes = bucket_num_bytes;
            self.buckets.remove(&bucket_key);

            // NOTE: Make sure the first bucket is responsible for `-∞`, which might or might not be
            // the case now that we've been moving buckets around.
            if let Some((_, bucket)) = self.buckets.pop_first() {
                self.buckets.insert(TimeInt::MIN, bucket);
            }
        }

        self.buckets_size_bytes -= dropped_num_bytes;
        self.buckets_num_rows -= (dropped_num_bytes > 0) as u64;

        (diff, dropped_num_bytes)
    }
}

impl IndexedBucketInner {
    /// Tries to drop the given `row_id` from the table, which is expected to be found at the
    /// specified `time`.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(
        &mut self,
        cluster_cache: &ClusterCellCache,
        cluster_key: ComponentName,
        row_id: RowId,
        timeline: Timeline,
        ent_path: &EntityPath,
        time: i64,
    ) -> (Option<StoreDiff>, u64) {
        self.sort();

        let IndexedBucketInner {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            max_row_id,
            col_num_instances,
            columns,
            size_bytes,
        } = self;

        let mut diff: Option<StoreDiff> = None;
        let mut dropped_num_bytes = 0u64;

        let mut row_index = col_time.partition_point(|&time2| time2 < time);
        while col_time.get(row_index) == Some(&time) {
            if col_row_id[row_index] != row_id {
                row_index += 1;
                continue;
            }

            // Update the time_range min/max:
            if col_time.len() == 1 {
                // We removed the last row
                *time_range = TimeRange::EMPTY;
            } else {
                *is_sorted = row_index == 0 || row_index.saturating_add(1) == col_row_id.len();

                // We have at least two rows, so we can safely [index] here:
                if row_index == 0 {
                    // We removed the first row, so the second row holds the new min
                    time_range.min = col_time[1].into();
                }
                if row_index + 1 == col_time.len() {
                    // We removed the last row, so the penultimate row holds the new max
                    time_range.max = col_time[row_index - 1].into();
                }
            }

            // col_row_id
            let Some(removed_row_id) = col_row_id.swap_remove(row_index) else {
                continue;
            };
            debug_assert_eq!(row_id, removed_row_id);
            dropped_num_bytes += removed_row_id.total_size_bytes();

            // col_time
            let row_time = col_time.swap_remove(row_index).unwrap();
            dropped_num_bytes += row_time.total_size_bytes();

            // col_insert_id (if present)
            if !col_insert_id.is_empty() {
                if let Some(insert_id) = col_insert_id.swap_remove(row_index) {
                    dropped_num_bytes += insert_id.total_size_bytes();
                }
            }

            // col_num_instances
            if let Some(num_instances) = col_num_instances.swap_remove(row_index) {
                dropped_num_bytes += num_instances.total_size_bytes();
            }

            // each data column
            for column in columns.values_mut() {
                let cell = column.0.swap_remove(row_index).flatten();

                // TODO(#1809): once datatype deduplication is in, we should really not count
                // autogenerated keys as part of the memory stats (same on write path).
                dropped_num_bytes += cell.total_size_bytes();

                if let Some(cell) = cell {
                    if cell.component_name() == cluster_key {
                        if let Some(cached_cell) = cluster_cache.get(&cell.num_instances()) {
                            if std::ptr::eq(cell.as_ptr(), cached_cell.as_ptr()) {
                                // We don't fire events when inserting autogenerated cluster cells, and
                                // therefore must not fire when removing them either.
                                continue;
                            }
                        }
                    }

                    if let Some(inner) = diff.as_mut() {
                        inner.cells.insert(cell.component_name(), cell);
                    } else {
                        let mut d = StoreDiff::deletion(removed_row_id, ent_path.clone());
                        d.at_timestamp(timeline, time).with_cells([cell]);
                        diff = Some(d);
                    }
                }
            }

            if *max_row_id == removed_row_id {
                // NOTE: We _have_ to fullscan here: the bucket is sorted by `(Time, RowId)`, there
                // could very well be a greater lurking in a lesser entry.
                *max_row_id = col_row_id.iter().max().copied().unwrap_or(RowId::ZERO);
            }

            // NOTE: A single `RowId` cannot possibly have more than one datapoint for
            // a single timeline.
            break;
        }

        *size_bytes -= dropped_num_bytes;

        (diff, dropped_num_bytes)
    }
}

impl PersistentIndexedTable {
    /// Tries to drop the given `row_id` from the table.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(
        &mut self,
        cluster_cache: &ClusterCellCache,
        row_id: RowId,
    ) -> (Option<StoreDiff>, u64) {
        re_tracing::profile_function!();

        let mut dropped_num_bytes = 0u64;

        let PersistentIndexedTable {
            ent_path,
            cluster_key: _,
            inner,
        } = self;

        let inner = &mut *inner.write();
        inner.sort();

        let PersistentIndexedTableInner {
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            is_sorted,
        } = inner;

        let mut diff: Option<StoreDiff> = None;

        if let Ok(row_index) = col_row_id.binary_search(&row_id) {
            *is_sorted = row_index == 0 || row_index.saturating_add(1) == col_row_id.len();

            // col_row_id
            let Some(removed_row_id) = col_row_id.swap_remove(row_index) else {
                return (None, 0);
            };
            debug_assert_eq!(row_id, removed_row_id);
            dropped_num_bytes += removed_row_id.total_size_bytes();

            // col_insert_id (if present)
            if !col_insert_id.is_empty() {
                if let Some(insert_id) = col_insert_id.swap_remove(row_index) {
                    dropped_num_bytes += insert_id.total_size_bytes();
                }
            }

            // col_num_instances
            if let Some(num_instances) = col_num_instances.swap_remove(row_index) {
                dropped_num_bytes += num_instances.total_size_bytes();
            }

            // each data column
            for column in columns.values_mut() {
                let cell = column.0.swap_remove(row_index).flatten();

                // TODO(#1809): once datatype deduplication is in, we should really not count
                // autogenerated keys as part of the memory stats (same on write path).
                dropped_num_bytes += cell.total_size_bytes();

                if let Some(cell) = cell {
                    if cell.component_name() == self.cluster_key {
                        if let Some(cached_cell) = cluster_cache.get(&cell.num_instances()) {
                            if std::ptr::eq(cell.as_ptr(), cached_cell.as_ptr()) {
                                // We don't fire events when inserting of autogenerated cluster cells, and
                                // therefore must not fire when removing them either.
                                continue;
                            }
                        }
                    }

                    if let Some(inner) = diff.as_mut() {
                        inner.cells.insert(cell.component_name(), cell);
                    } else {
                        let mut d = StoreDiff::deletion(removed_row_id, ent_path.clone());
                        d.cells.insert(cell.component_name(), cell);
                        diff = Some(d);
                    }
                }
            }
        }

        (diff, dropped_num_bytes)
    }
}
