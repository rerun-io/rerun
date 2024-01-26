use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntMap;
use parking_lot::{Mutex, RwLock};

use re_data_store::{
    DataStore, DataStoreConfig, GarbageCollectionOptions, StoreEvent, StoreSubscriber,
};
use re_log_types::{
    ApplicationId, DataCell, DataRow, DataTable, EntityPath, EntityPathHash, LogMsg, RowId,
    SetStoreInfo, StoreId, StoreInfo, StoreKind, TableId, TimePoint, Timeline,
};
use re_types_core::{components::InstanceKey, Archetype, Loggable};

use crate::{ClearCascade, CompactedStoreEvents, Error, TimesPerTimeline};

// ---

/// See [`DataStore::insert_row_with_retries`].
const MAX_INSERT_ROW_ATTEMPTS: usize = 1_000;

/// See [`DataStore::insert_row_with_retries`].
const DEFAULT_INSERT_ROW_STEP_SIZE: u64 = 100;

/// See [`GarbageCollectionOptions::time_budget`].
const DEFAULT_GC_TIME_BUDGET: std::time::Duration = std::time::Duration::from_micros(3500); // empirical

// ----------------------------------------------------------------------------

/// An in-memory database built from a stream of [`LogMsg`]es.
///
/// NOTE: all mutation is to be done via public functions!
pub struct EntityDb {
    /// The [`StoreId`] for this log.
    store_id: StoreId,

    /// Set by whomever created this [`EntityDb`].
    pub data_source: Option<re_smart_channel::SmartChannelSource>,

    /// Comes in a special message, [`LogMsg::SetStoreInfo`].
    set_store_info: Option<SetStoreInfo>,

    /// Keeps track of the last time data was inserted into this store (viewer wall-clock).
    last_modified_at: web_time::Instant,

    /// In many places we just store the hashes, so we need a way to translate back.
    entity_path_from_hash: IntMap<EntityPathHash, EntityPath>,

    /// The global-scope time tracker.
    ///
    /// For each timeline, keeps track of what times exist, recursively across all
    /// entities/components.
    ///
    /// Used for time control.
    times_per_timeline: TimesPerTimeline,

    /// A tree-view (split on path components) of the entities.
    tree: crate::EntityTree,

    /// Stores all components for all entities for all timelines.
    // TODO: introduce a DataStore?
    // TODO: why we shard here
    data_store: DataStore,

    /// Query caches for the data in [`Self::data_store`].
    query_caches: re_query_cache::Caches,

    stats: IngestionStatistics,
}

impl EntityDb {
    pub fn new(store_id: StoreId) -> Self {
        let data_store = DataStore::new(
            store_id.clone(),
            InstanceKey::name(),
            DataStoreConfig::default(),
        );
        let query_caches = re_query_cache::Caches::from_store_id(store_id.clone());
        Self {
            store_id: store_id.clone(),
            data_source: None,
            set_store_info: None,
            last_modified_at: web_time::Instant::now(),
            entity_path_from_hash: Default::default(),
            times_per_timeline: Default::default(),
            tree: crate::EntityTree::root(),
            data_store,
            query_caches,
            stats: IngestionStatistics::new(store_id),
        }
    }

    /// Helper function to create a recording from a [`StoreInfo`] and some [`DataRow`]s.
    ///
    /// This is useful to programmatically create recordings from within the viewer, which cannot
    /// use the `re_sdk`, which is not Wasm-compatible.
    pub fn from_info_and_rows(
        store_info: StoreInfo,
        rows: impl IntoIterator<Item = DataRow>,
    ) -> Result<Self, Error> {
        let mut entity_db = EntityDb::new(store_info.store_id.clone());

        entity_db.set_store_info(SetStoreInfo {
            row_id: RowId::new(),
            info: store_info,
        });

        let table = DataTable::from_rows(TableId::new(), rows);
        entity_db.add_data_table(table)?;

        Ok(entity_db)
    }

    #[inline]
    pub fn tree(&self) -> &crate::EntityTree {
        &self.tree
    }

    pub fn store_info_msg(&self) -> Option<&SetStoreInfo> {
        self.set_store_info.as_ref()
    }

    pub fn store_info(&self) -> Option<&StoreInfo> {
        self.store_info_msg().map(|msg| &msg.info)
    }

    pub fn app_id(&self) -> Option<&ApplicationId> {
        self.store_info().map(|ri| &ri.application_id)
    }

    #[inline]
    pub fn query_caches(&self) -> &re_query_cache::Caches {
        &self.query_caches
    }

    #[inline]
    pub fn store(&self) -> &DataStore {
        &self.data_store
    }

    pub fn store_kind(&self) -> StoreKind {
        self.store_id.kind
    }

    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.times_per_timeline().keys()
    }

    pub fn times_per_timeline(&self) -> &TimesPerTimeline {
        &self.times_per_timeline
    }

    /// Histogram of all events on the timeeline, of all entities.
    pub fn time_histogram(&self, timeline: &Timeline) -> Option<&crate::TimeHistogram> {
        self.tree().subtree.time_histogram.get(timeline)
    }

    /// Total number of timeless messages for any entity.
    pub fn num_timeless_messages(&self) -> u64 {
        self.tree.num_timeless_messages_recursive()
    }

    pub fn num_rows(&self) -> usize {
        (self.data_store.num_timeless_rows() + self.data_store.num_temporal_rows()) as usize
    }

    /// Return the current `StoreGeneration`. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    pub fn generation(&self) -> re_data_store::StoreGeneration {
        self.data_store.generation()
    }

    pub fn last_modified_at(&self) -> web_time::Instant {
        self.last_modified_at
    }

    pub fn is_empty(&self) -> bool {
        self.set_store_info.is_none() && self.num_rows() == 0
    }

    /// A sorted list of all the entity paths in this database.
    pub fn entity_paths(&self) -> Vec<&EntityPath> {
        use itertools::Itertools as _;
        self.entity_path_from_hash.values().sorted().collect()
    }

    #[inline]
    pub fn ingestion_stats(&self) -> &IngestionStatistics {
        &self.stats
    }

    #[inline]
    pub fn entity_path_from_hash(&self, entity_path_hash: &EntityPathHash) -> Option<&EntityPath> {
        self.entity_path_from_hash.get(entity_path_hash)
    }

    /// Returns `true` also for entities higher up in the hierarchy.
    #[inline]
    pub fn is_known_entity(&self, entity_path: &EntityPath) -> bool {
        self.tree.subtree(entity_path).is_some()
    }

    /// If you log `world/points`, then that is a logged entity, but `world` is not,
    /// unless you log something to `world` too.
    #[inline]
    pub fn is_logged_entity(&self, entity_path: &EntityPath) -> bool {
        self.entity_path_from_hash.contains_key(&entity_path.hash())
    }

    pub fn add(&mut self, msg: &LogMsg) -> Result<(), Error> {
        re_tracing::profile_function!();

        debug_assert_eq!(msg.store_id(), self.store_id());

        match &msg {
            LogMsg::SetStoreInfo(msg) => self.set_store_info(msg.clone()),

            LogMsg::ArrowMsg(_, arrow_msg) => {
                let table = DataTable::from_arrow_msg(arrow_msg)?;
                // self.add_data_table(table)?;
                self.add_data_table(table)?;
            }
        }

        Ok(())
    }

    // TODO: don't call this in a loop
    // TODO: this exists only for tests right? yes, pretty much
    #[inline]
    pub fn add_data_row(&mut self, row: DataRow) -> Result<(), Error> {
        self.add_data_table(DataTable::from_rows(TableId::new(), [row]))
    }

    pub fn add_data_table(&mut self, mut table: DataTable) -> Result<(), Error> {
        re_tracing::profile_function!(format!("num_rows={}", table.num_rows()));

        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        table.compute_all_size_bytes();

        for row in table.to_rows() {
            let row = row?;
            self.register_entity_path(row.entity_path());
        }

        let store_events = self.data_store.insert_table_with_retries(
            table,
            MAX_INSERT_ROW_ATTEMPTS,
            DEFAULT_INSERT_ROW_STEP_SIZE,
        )?;

        // First-pass: update our internal views by notifying them of resulting [`StoreEvent`]s.
        //
        // This might result in a [`ClearCascade`] if the events trigger one or more immediate
        // and/or pending clears.
        let original_store_events = &store_events;
        self.times_per_timeline.on_events(original_store_events);
        self.query_caches.on_events(original_store_events);
        let clear_cascade = self.tree.on_store_additions(original_store_events);

        // Second-pass: update the [`DataStore`] by applying the [`ClearCascade`].
        //
        // This will in turn generate new [`StoreEvent`]s that our internal views need to be
        // notified of, again!
        let new_store_events = self.on_clear_cascade(clear_cascade);
        self.times_per_timeline.on_events(&new_store_events);
        self.query_caches.on_events(&new_store_events);
        let clear_cascade = self.tree.on_store_additions(&new_store_events);

        // Clears don't affect `Clear` components themselves, therefore we cannot have recursive
        // cascades, thus this whole process must stabilize after one iteration.
        debug_assert!(clear_cascade.is_empty());

        // We inform the stats last, since it measures e2e latency.
        self.stats.on_events(original_store_events);

        self.last_modified_at = web_time::Instant::now();

        Ok(())
    }

    fn on_clear_cascade(&mut self, clear_cascade: ClearCascade) -> Vec<StoreEvent> {
        let mut store_events = Vec::new();

        // Create the empty cells to be inserted.
        //
        // Reminder: these are the [`RowId`]s of the `Clear` components that triggered the
        // cascade, they are not unique and may be shared across many entity paths.
        let mut to_be_inserted =
            BTreeMap::<RowId, BTreeMap<EntityPath, (TimePoint, Vec<DataCell>)>>::default();
        for (row_id, per_entity) in clear_cascade.to_be_cleared {
            for (entity_path, (timepoint, component_paths)) in per_entity {
                let per_entity = to_be_inserted.entry(row_id).or_default();
                let (cur_timepoint, cells) = per_entity.entry(entity_path).or_default();

                *cur_timepoint = timepoint.union_max(cur_timepoint);

                for component_path in component_paths {
                    if let Some(data_type) = self
                        .data_store
                        .lookup_datatype(&component_path.component_name)
                    {
                        cells.push(DataCell::from_arrow_empty(
                            component_path.component_name,
                            data_type.clone(),
                        ));
                    }
                }
            }
        }

        for (row_id, per_entity) in to_be_inserted {
            let mut row_id = row_id;
            for (entity_path, (timepoint, cells)) in per_entity {
                // NOTE: It is important we insert all those empty components using a single row (id)!
                // 1. It'll be much more efficient when querying that data back.
                // 2. Otherwise we will end up with a flaky row ordering, as we have no way to tie-break
                //    these rows! This flaky ordering will in turn leak through the public
                //    API (e.g. range queries)!
                match DataRow::from_cells(row_id, timepoint.clone(), entity_path, 0, cells) {
                    Ok(row) => {
                        let res = self.data_store.insert_row_with_retries(
                            row,
                            MAX_INSERT_ROW_ATTEMPTS,
                            DEFAULT_INSERT_ROW_STEP_SIZE,
                        );

                        match res {
                            Ok(store_event) => {
                                row_id = store_event.row_id.next();
                                store_events.push(store_event);
                            }
                            Err(err) => {
                                re_log::error_once!(
                                    "Failed to propagate EntityTree cascade: {err}"
                                );
                            }
                        }
                    }
                    Err(err) => {
                        re_log::error_once!("Failed to propagate EntityTree cascade: {err}");
                    }
                }
            }
        }

        store_events
    }

    fn register_entity_path(&mut self, entity_path: &EntityPath) {
        self.entity_path_from_hash
            .entry(entity_path.hash())
            .or_insert_with(|| entity_path.clone());
    }

    pub fn set_store_info(&mut self, store_info: SetStoreInfo) {
        self.set_store_info = Some(store_info);
    }

    pub fn gc_everything_but_the_latest_row(&mut self) {
        re_tracing::profile_function!();

        self.gc(&GarbageCollectionOptions {
            target: re_data_store::GarbageCollectionTarget::Everything,
            gc_timeless: true,
            protect_latest: 1, // TODO(jleibs): Bump this after we have an undo buffer
            purge_empty_tables: true,
            dont_protect: [
                re_types_core::components::ClearIsRecursive::name(),
                re_types_core::archetypes::Clear::indicator().name(),
            ]
            .into_iter()
            .collect(),
            enable_batching: false,
            time_budget: DEFAULT_GC_TIME_BUDGET,
        });
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        assert!((0.0..=1.0).contains(&fraction_to_purge));
        self.gc(&GarbageCollectionOptions {
            target: re_data_store::GarbageCollectionTarget::DropAtLeastFraction(
                fraction_to_purge as _,
            ),
            gc_timeless: true,
            protect_latest: 1,
            purge_empty_tables: false,
            dont_protect: Default::default(),
            enable_batching: false,
            time_budget: DEFAULT_GC_TIME_BUDGET,
        });
    }

    pub fn gc(&mut self, gc_options: &GarbageCollectionOptions) {
        re_tracing::profile_function!();

        // TODO: shit, somehow need to split that evenly

        // let (store_events, stats_diff) = self.data_store.gc(gc_options);
        //
        // re_log::trace!(
        //     num_row_ids_dropped = store_events.len(),
        //     size_bytes_dropped = re_format::format_bytes(stats_diff.total.num_bytes as _),
        //     "purged datastore"
        // );
        //
        // self.on_store_deletions(&store_events);
    }

    fn on_store_deletions(&mut self, store_events: &[StoreEvent]) {
        re_tracing::profile_function!();

        let Self {
            store_id: _,
            data_source: _,
            set_store_info: _,
            last_modified_at: _,
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _,
            query_caches,
            stats: _,
        } = self;

        times_per_timeline.on_events(store_events);
        query_caches.on_events(store_events);

        let store_events = store_events.iter().collect_vec();
        let compacted = CompactedStoreEvents::new(&store_events);
        tree.on_store_deletions(&store_events, &compacted);
    }

    /// Key used for sorting recordings in the UI.
    pub fn sort_key(&self) -> impl Ord + '_ {
        self.store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }
}

// ----------------------------------------------------------------------------

pub struct IngestionStatistics {
    store_id: StoreId,
    e2e_latency_sec_history: Mutex<emath::History<f32>>,
}

impl StoreSubscriber for IngestionStatistics {
    fn name(&self) -> String {
        "rerun.testing.store_subscribers.IngestionStatistics".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            if event.store_id == self.store_id {
                self.on_new_row_id(event.row_id);
            }
        }
    }
}

impl IngestionStatistics {
    pub fn new(store_id: StoreId) -> Self {
        let min_samples = 0; // 0: we stop displaying e2e latency if input stops
        let max_samples = 1024; // don't waste too much memory on this - we just need enough to get a good average
        let max_age = 1.0; // don't keep too long of a rolling average, or the stats get outdated.
        Self {
            store_id,
            e2e_latency_sec_history: Mutex::new(emath::History::new(
                min_samples..max_samples,
                max_age,
            )),
        }
    }

    fn on_new_row_id(&mut self, row_id: RowId) {
        if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
            let nanos_since_epoch = duration_since_epoch.as_nanos() as u64;

            // This only makes sense if the clocks are very good, i.e. if the recording was on the same machine!
            if let Some(nanos_since_log) =
                nanos_since_epoch.checked_sub(row_id.nanoseconds_since_epoch())
            {
                let now = nanos_since_epoch as f64 / 1e9;
                let sec_since_log = nanos_since_log as f32 / 1e9;

                self.e2e_latency_sec_history.lock().add(now, sec_since_log);
            }
        }
    }

    /// What is the mean latency between the time data was logged in the SDK and the time it was ingested?
    ///
    /// This is based on the clocks of the viewer and the SDK being in sync,
    /// so if the recording was done on another machine, this is likely very inaccurate.
    pub fn current_e2e_latency_sec(&self) -> Option<f32> {
        let mut e2e_latency_sec_history = self.e2e_latency_sec_history.lock();

        if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
            let nanos_since_epoch = duration_since_epoch.as_nanos() as u64;
            let now = nanos_since_epoch as f64 / 1e9;
            e2e_latency_sec_history.flush(now); // make sure the average is up-to-date.
        }

        e2e_latency_sec_history.average()
    }
}
