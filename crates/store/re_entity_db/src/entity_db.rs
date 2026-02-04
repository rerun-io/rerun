use std::{
    collections::BTreeSet,
    fmt::{Debug, Formatter},
    ops::Deref,
    sync::Arc,
};

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture, SizeBytes as _};
use re_chunk::{
    Chunk, ChunkBuilder, ChunkId, ChunkResult, ComponentIdentifier, LatestAtQuery, RowId, TimeInt,
    TimePoint, Timeline, TimelineName,
};
use re_chunk_store::{
    ChunkStore, ChunkStoreChunkStats, ChunkStoreConfig, ChunkStoreEvent, ChunkStoreHandle,
    ChunkStoreSubscriber as _, GarbageCollectionOptions, GarbageCollectionTarget,
};
use re_log_channel::LogSource;
use re_log_encoding::RrdManifest;
use re_log_types::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, ApplicationId, EntityPath, EntityPathHash, LogMsg,
    RecordingId, SetStoreInfo, StoreId, StoreInfo, StoreKind, TimeType,
};
use re_mutex::Mutex;
use re_query::{
    QueryCache, QueryCacheHandle, StorageEngine, StorageEngineArcReadGuard, StorageEngineReadGuard,
};

use crate::ingestion_statistics::IngestionStatistics;
use crate::rrd_manifest_index::RrdManifestIndex;
use crate::{Error, TimeHistogramPerTimeline};

// ----------------------------------------------------------------------------

/// See [`GarbageCollectionOptions::time_budget`].
pub const DEFAULT_GC_TIME_BUDGET: std::time::Duration = std::time::Duration::from_micros(3500); // empirical

// ----------------------------------------------------------------------------¨

/// What class of [`EntityDb`] is this?
///
/// The class is used to semantically group recordings in the UI (e.g. in the recording panel) and
/// to determine how to source the default blueprint. For example, `DatasetSegment` dbs might have
/// their default blueprint sourced remotely.
#[derive(Debug, PartialEq, Eq)]
pub enum EntityDbClass<'a> {
    /// This is a regular local recording (e.g. loaded from a `.rrd` file or logged to the viewer).
    LocalRecording,

    /// This is an official rerun example recording.
    ExampleRecording,

    /// This is a recording loaded from a remote dataset segment.
    DatasetSegment(&'a re_uri::DatasetSegmentUri),

    /// This is a blueprint.
    Blueprint,
}

impl EntityDbClass<'_> {
    pub fn is_example(&self) -> bool {
        matches!(self, EntityDbClass::ExampleRecording)
    }
}

// ---

struct StoreSizeBytes(Mutex<Option<u64>>);

impl Clone for StoreSizeBytes {
    fn clone(&self) -> Self {
        Self(Mutex::new(*self.0.lock()))
    }
}

impl Deref for StoreSizeBytes {
    type Target = Mutex<Option<u64>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ---

/// An in-memory database built from a stream of [`LogMsg`]es.
///
/// NOTE: all mutation is to be done via public functions!
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct EntityDb {
    /// Store id associated with this [`EntityDb`]. Must be identical to the `storage_engine`'s store id.
    store_id: StoreId,

    /// Whether the `EntityDb` should maintain various secondary indexes using store events.
    ///
    /// These indexes are costly to maintain and only useful when running in the viewer.
    /// For CLI tools, prefer disabling this to improve performance, unless you specifically need
    /// these indexes for some reason.
    enable_viewer_indexes: bool,

    /// Set by whomever created this [`EntityDb`].
    ///
    /// Clones of an [`EntityDb`] gets a `None` source.
    pub data_source: Option<re_log_channel::LogSource>,

    pub rrd_manifest_index: RrdManifestIndex,

    /// Comes in a special message, [`LogMsg::SetStoreInfo`].
    set_store_info: Option<SetStoreInfo>,

    /// Keeps track of the last time data was inserted into this store (viewer wall-clock).
    last_modified_at: web_time::Instant,

    /// The highest `RowId` in the store,
    /// which corresponds to the last edit time.
    /// Ignores deletions.
    latest_row_id: Option<RowId>,

    /// All the entity paths in this database, sorted for use in GUIs.
    entity_paths: BTreeSet<EntityPath>,

    /// In many places we just store the hashes, so we need a way to translate back.
    entity_path_from_hash: IntMap<EntityPathHash, EntityPath>,

    /// A time histogram of all entities, for every timeline.
    time_histogram_per_timeline: crate::TimeHistogramPerTimeline,

    /// The [`StorageEngine`] that backs this [`EntityDb`].
    ///
    /// This object and all its internal fields are **never** allowed to be publicly exposed,
    /// whether that is directly or through methods, _even if that's just shared references_.
    ///
    /// The only way to get access to the [`StorageEngine`] from the outside is to use
    /// [`EntityDb::storage_engine`], which returns a read-only guard.
    /// The design statically guarantees the absence of deadlocks and race conditions that normally
    /// results from letting store and cache handles arbitrarily loose all across the codebase.
    storage_engine: StorageEngine,

    /// Lazily calculated
    store_size_bytes: StoreSizeBytes,

    stats: IngestionStatistics,
}

impl Debug for EntityDb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityDb")
            .field("enable_viewer_indexes", &self.enable_viewer_indexes)
            .field("store_id", &self.store_id)
            .field("data_source", &self.data_source)
            .field("set_store_info", &self.set_store_info)
            .finish()
    }
}

impl EntityDb {
    /// [Secondary viewer indexes] are enabled by default.
    ///
    /// Use [`Self::with_store_config`] for more control.
    ///
    /// [Secondary viewer indexes]: [`Self::enable_viewer_indexes`]
    pub fn new(store_id: StoreId) -> Self {
        let enable_viewer_indexes = true;
        Self::with_store_config(
            store_id,
            enable_viewer_indexes,
            ChunkStoreConfig::from_env().unwrap_or_default(),
        )
    }

    pub fn with_store_config(
        store_id: StoreId,
        enable_viewer_indexes: bool,
        mut store_config: ChunkStoreConfig,
    ) -> Self {
        // If we don't care about inline indexes, we definitely don't care about remote subscribers either.
        store_config.enable_changelog = enable_viewer_indexes;

        let store = ChunkStoreHandle::new(ChunkStore::new(store_id.clone(), store_config));
        let cache = QueryCacheHandle::new(QueryCache::new(store.clone()));

        // Safety: these handles are never going to be leaked outside of the `EntityDb`.
        #[expect(unsafe_code)]
        let storage_engine = unsafe { StorageEngine::new(store, cache) };

        Self {
            store_id,
            enable_viewer_indexes,
            data_source: None,
            rrd_manifest_index: Default::default(),
            set_store_info: None,
            last_modified_at: web_time::Instant::now(),
            latest_row_id: None,
            entity_paths: Default::default(),
            entity_path_from_hash: Default::default(),
            time_histogram_per_timeline: Default::default(),
            storage_engine,
            store_size_bytes: StoreSizeBytes(Mutex::new(None)),
            stats: IngestionStatistics::default(),
        }
    }

    #[inline]
    pub fn tree(&self) -> &crate::EntityTree {
        &self.rrd_manifest_index.entity_tree
    }

    /// Formats the entity tree into a human-readable text representation with component schema information.
    pub fn format_with_components(&self) -> String {
        let mut text = String::new();

        let storage_engine = self.storage_engine();
        let store = storage_engine.store();

        self.tree().visit_children_recursively(|entity_path| {
            if entity_path.is_root() {
                return;
            }
            let depth = entity_path.len() - 1;
            let indent = "  ".repeat(depth);
            text.push_str(&format!("{indent}{entity_path}\n"));
            let Some(components) = store.all_components_for_entity_sorted(entity_path) else {
                return;
            };
            for component in components {
                let component_indent = "  ".repeat(depth + 1);
                if let Some(component_descr) =
                    store.entity_component_descriptor(entity_path, component)
                    && let Some(component_type) = &component_descr.component_type
                {
                    if let Some(datatype) = store.lookup_datatype(component_type) {
                        text.push_str(&format!(
                            "{}{}: {}\n",
                            component_indent,
                            component_type.short_name(),
                            re_arrow_util::format_data_type(&datatype)
                        ));
                    } else {
                        text.push_str(&format!(
                            "{}{}\n",
                            component_indent,
                            component_type.short_name()
                        ));
                    }
                } else {
                    // Fallback to component identifier
                    text.push_str(&format!("{component_indent}{component}\n"));
                }
            }
        });
        text
    }

    /// Returns a read-only guard to the backing [`StorageEngine`].
    #[inline]
    pub fn storage_engine(&self) -> StorageEngineReadGuard<'_> {
        self.storage_engine.read()
    }

    pub fn rrd_manifest_index_mut_and_storage_engine(
        &mut self,
    ) -> (&mut RrdManifestIndex, StorageEngineReadGuard<'_>) {
        (&mut self.rrd_manifest_index, self.storage_engine.read())
    }

    /// Returns a reference to the backing [`StorageEngine`].
    ///
    /// This can be used to obtain a clone of the [`StorageEngine`].
    ///
    /// # Safety
    ///
    /// Trying to lock the [`StorageEngine`] (whether read or write) while the computation of a viewer's
    /// frame is already in progress will lead to data inconsistencies, livelocks and deadlocks.
    /// The viewer runs a synchronous work-stealing scheduler (`rayon`) as well as an asynchronous
    /// one (`tokio`): when and where locks are taken is entirely non-deterministic (even unwanted reentrancy
    /// is a possibility).
    ///
    /// Don't use this unless you know what you're doing. Use [`Self::storage_engine`] instead.
    #[expect(unsafe_code)]
    pub unsafe fn storage_engine_raw(&self) -> &StorageEngine {
        &self.storage_engine
    }

    /// Returns a read-only guard to the backing [`StorageEngine`].
    ///
    /// That guard can be cloned at will and has a static lifetime.
    ///
    /// It is not possible to insert any more data in this [`EntityDb`] until the returned guard,
    /// and any clones, have been dropped.
    #[inline]
    pub fn storage_engine_arc(&self) -> StorageEngineArcReadGuard {
        self.storage_engine.read_arc()
    }

    #[inline]
    pub fn rrd_manifest_index(&self) -> &RrdManifestIndex {
        &self.rrd_manifest_index
    }

    #[inline]
    pub fn rrd_manifest_index_mut(&mut self) -> &mut RrdManifestIndex {
        &mut self.rrd_manifest_index
    }

    #[inline]
    pub fn store_info_msg(&self) -> Option<&SetStoreInfo> {
        self.set_store_info.as_ref()
    }

    #[inline]
    pub fn store_info(&self) -> Option<&StoreInfo> {
        self.store_info_msg().map(|msg| &msg.info)
    }

    #[inline]
    pub fn application_id(&self) -> &ApplicationId {
        self.store_id().application_id()
    }

    #[inline]
    pub fn recording_id(&self) -> &RecordingId {
        self.store_id().recording_id()
    }

    #[inline]
    pub fn store_kind(&self) -> StoreKind {
        self.store_id().kind()
    }

    #[inline]
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// What redap URI does this thing live on?
    pub fn redap_uri(&self) -> Option<&re_uri::DatasetSegmentUri> {
        if let Some(re_log_channel::LogSource::RedapGrpcStream { uri, .. }) = &self.data_source {
            Some(uri)
        } else {
            None
        }
    }

    /// Returns the [`EntityDbClass`] of this entity db.
    pub fn store_class(&self) -> EntityDbClass<'_> {
        match self.store_kind() {
            StoreKind::Blueprint => EntityDbClass::Blueprint,

            StoreKind::Recording => match &self.data_source {
                Some(LogSource::RrdHttpStream { url, .. })
                    if url.starts_with("https://app.rerun.io") =>
                {
                    EntityDbClass::ExampleRecording
                }

                Some(LogSource::RedapGrpcStream { uri, .. }) => EntityDbClass::DatasetSegment(uri),

                _ => EntityDbClass::LocalRecording,
            },
        }
    }

    /// Read one of the built-in `RecordingInfo` properties.
    pub fn recording_info_property<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<C> {
        debug_assert!(
            component.starts_with("RecordingInfo:"),
            "This function should only be used for built-in RecordingInfo components, which are the only recording properties at {}",
            EntityPath::properties()
        );

        self.latest_at_component::<C>(
            &EntityPath::properties(),
            &LatestAtQuery::latest(TimelineName::log_tick()),
            component,
        )
        .map(|(_, value)| value)
    }

    /// Use can use this both for setting the built-in `RecordingInfo` components,
    /// and for setting custom properties on the recording.
    pub fn set_recording_property<Component: re_types_core::Component>(
        &mut self,
        entity_path: EntityPath,
        component_descr: re_types_core::ComponentDescriptor,
        value: &Component,
    ) -> Result<(), Error> {
        debug_assert_eq!(component_descr.component_type, Some(Component::name()));
        debug_assert!(entity_path.starts_with(&EntityPath::properties()));
        debug_assert!(
            (entity_path == EntityPath::properties())
                == (component_descr.archetype == Some("rerun.archetypes.RecordingInfo".into())),
            "RecordingInfo should be logged at {}. Custom properties should be under a child entity",
            EntityPath::properties()
        );

        let chunk = ChunkBuilder::new(ChunkId::new(), entity_path)
            .with_component(RowId::new(), TimePoint::STATIC, component_descr, value)
            .map_err(|err| Error::Chunk(err.into()))?
            .build()?;

        self.add_chunk(&Arc::new(chunk))?;

        Ok(())
    }

    pub fn timeline_type(&self, timeline_name: &TimelineName) -> TimeType {
        self.storage_engine()
            .store()
            .time_column_type(timeline_name)
            .unwrap_or_else(|| {
                if timeline_name == &TimelineName::log_time() {
                    Timeline::log_time().typ()
                } else if timeline_name == &TimelineName::log_tick() {
                    Timeline::log_tick().typ()
                } else {
                    re_log::warn_once!("Timeline {timeline_name:?} not found");
                    TimeType::Sequence
                }
            })
    }

    /// Queries for the given components using latest-at semantics.
    ///
    /// See [`re_query::LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    #[inline]
    pub fn latest_at(
        &self,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> re_query::LatestAtResults {
        self.storage_engine
            .read()
            .cache()
            .latest_at(query, entity_path, components)
    }

    /// Get the latest index and value for a given dense [`re_types_core::Component`].
    ///
    /// This assumes that the row we get from the store contains at most one instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    #[inline]
    pub fn latest_at_component<C: re_types_core::Component>(
        &self,
        entity_path: &EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        component: ComponentIdentifier,
    ) -> Option<((TimeInt, RowId), C)> {
        let results = self
            .storage_engine
            .read()
            .cache()
            .latest_at(query, entity_path, [component]);
        results
            .component_mono(component)
            .map(|value| (results.max_index(), value))
    }

    /// Get the latest index and value for a given dense [`re_types_core::Component`].
    ///
    /// This assumes that the row we get from the store contains at most one instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, and will quietly swallow any errors.
    #[inline]
    pub fn latest_at_component_quiet<C: re_types_core::Component>(
        &self,
        entity_path: &EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        component: ComponentIdentifier,
    ) -> Option<((TimeInt, RowId), C)> {
        let results = self
            .storage_engine
            .read()
            .cache()
            .latest_at(query, entity_path, [component]);

        results
            .component_mono_quiet(component)
            .map(|value| (results.max_index(), value))
    }

    #[inline]
    pub fn latest_at_component_at_closest_ancestor<C: re_types_core::Component>(
        &self,
        entity_path: &EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        component: ComponentIdentifier,
    ) -> Option<(EntityPath, (TimeInt, RowId), C)> {
        re_tracing::profile_function!();

        let mut cur_entity_path = Some(entity_path.clone());
        while let Some(entity_path) = cur_entity_path {
            if let Some((index, value)) = self.latest_at_component(&entity_path, query, component) {
                return Some((entity_path, index, value));
            }
            cur_entity_path = entity_path.parent();
        }

        None
    }

    /// Check if we have all loaded chunk for the given entity and component at `query.at()`.
    pub fn has_fully_loaded(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
        query: &LatestAtQuery,
    ) -> bool {
        !self
            .rrd_manifest_index()
            .unloaded_temporal_entries_for(&query.timeline(), entity_path, Some(component))
            .any(|chunk| chunk.time_range.contains(query.at()))
    }

    /// If this entity db is the result of a clone, which store was it cloned from?
    ///
    /// A cloned store always gets a new unique ID.
    ///
    /// We currently only use entity db cloning for blueprints:
    /// when we activate a _default_ blueprint that was received on the wire (e.g. from a recording),
    /// we clone it and make the clone the _active_ blueprint.
    /// This means all active blueprints are clones.
    #[inline]
    pub fn cloned_from(&self) -> Option<&StoreId> {
        let info = self.store_info()?;
        info.cloned_from.as_ref()
    }

    pub fn timelines(&self) -> std::collections::BTreeMap<TimelineName, Timeline> {
        self.storage_engine().store().timelines()
    }

    /// When do we have data on each timeline?
    pub fn timeline_histograms(&self) -> &TimeHistogramPerTimeline {
        &self.time_histogram_per_timeline
    }

    /// Returns the time range of data on the given timeline, ignoring any static times.
    pub fn time_range_for(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        self.storage_engine().store().time_range(timeline)
    }

    /// Histogram of all events on the timeeline, of all entities.
    pub fn time_histogram(&self, timeline: &TimelineName) -> Option<&crate::TimeHistogram> {
        self.time_histogram_per_timeline.get(timeline)
    }

    #[inline]
    pub fn num_rows(&self) -> u64 {
        self.storage_engine.read().store().stats().total().num_rows
    }

    /// Return the current `ChunkStoreGeneration`. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    #[inline]
    pub fn generation(&self) -> re_chunk_store::ChunkStoreGeneration {
        self.storage_engine.read().store().generation()
    }

    #[inline]
    pub fn last_modified_at(&self) -> web_time::Instant {
        self.last_modified_at
    }

    /// The highest `RowId` in the store,
    /// which corresponds to the last edit time.
    /// Ignores deletions.
    #[inline]
    pub fn latest_row_id(&self) -> Option<RowId> {
        self.latest_row_id
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.set_store_info.is_none() && self.num_rows() == 0
    }

    /// A sorted list of all the entity paths in this database.
    pub fn sorted_entity_paths(&self) -> impl Iterator<Item = &EntityPath> {
        self.entity_paths.iter()
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
        self.tree().subtree(entity_path).is_some()
    }

    /// If you log `world/points`, then that is a logged entity, but `world` is not,
    /// unless you log something to `world` too.
    #[inline]
    pub fn is_logged_entity(&self, entity_path: &EntityPath) -> bool {
        self.entity_path_from_hash.contains_key(&entity_path.hash())
    }

    pub fn add_rrd_manifest_message(&mut self, rrd_manifest: Arc<RrdManifest>) {
        re_tracing::profile_function!();
        re_log::debug!("Received RrdManifest for {:?}", self.store_id());

        if let Err(err) = self
            .storage_engine
            .write()
            .store()
            .insert_rrd_manifest(rrd_manifest.clone())
        {
            re_log::error!("Failed to load RRD Manifest into store: {err}");
        }

        if let Err(err) = self.rrd_manifest_index.append(rrd_manifest) {
            re_log::error!("Failed to load RRD Manifest: {err}");
        }

        self.time_histogram_per_timeline
            .on_rrd_manifest(&self.rrd_manifest_index);
    }

    /// Insert new data into the store.
    pub fn add_log_msg(&mut self, msg: &LogMsg) -> Result<Vec<ChunkStoreEvent>, Error> {
        debug_assert_eq!(msg.store_id(), self.store_id());

        match &msg {
            LogMsg::SetStoreInfo(msg) => {
                self.set_store_info(msg.clone());
                Ok(vec![]) // no events
            }

            LogMsg::ArrowMsg(_, arrow_msg) => self.add_record_batch(&arrow_msg.batch),

            LogMsg::BlueprintActivationCommand(_) => {
                // Not for us to handle
                Ok(vec![]) // no events
            }
        }
    }

    /// Insert a chunk (encoded as a record batch) into the store.
    pub fn add_record_batch(
        &mut self,
        record_batch: &arrow::array::RecordBatch,
    ) -> Result<Vec<ChunkStoreEvent>, Error> {
        re_tracing::profile_function!(format!(
            "{} rows",
            re_format::format_uint(record_batch.num_rows())
        ));

        self.last_modified_at = web_time::Instant::now();
        let chunk_batch =
            re_sorbet::ChunkBatch::try_from(record_batch).map_err(re_chunk::ChunkError::from)?;
        let mut chunk = re_chunk::Chunk::from_chunk_batch(&chunk_batch)?;
        chunk.sort_if_unsorted();
        self.add_chunk_with_timestamp_metadata(
            &Arc::new(chunk),
            &chunk_batch.sorbet_schema().timestamps,
        )
    }

    /// Insert new data into the store.
    pub fn add_chunk(&mut self, chunk: &Arc<Chunk>) -> Result<Vec<ChunkStoreEvent>, Error> {
        re_tracing::profile_function!();
        self.add_chunk_with_timestamp_metadata(chunk, &Default::default())
    }

    fn add_chunk_with_timestamp_metadata(
        &mut self,
        chunk: &Arc<Chunk>,
        chunk_timestamps: &re_sorbet::TimestampMetadata,
    ) -> Result<Vec<ChunkStoreEvent>, Error> {
        let store_events = self.storage_engine.write().store().insert_chunk(chunk)?;

        self.entity_paths.insert(chunk.entity_path().clone());

        self.entity_path_from_hash
            .entry(chunk.entity_path().hash())
            .or_insert_with(|| chunk.entity_path().clone());

        if self.latest_row_id < chunk.row_id_range().map(|(_, row_id_max)| row_id_max) {
            self.latest_row_id = chunk.row_id_range().map(|(_, row_id_max)| row_id_max);
        }

        self.rrd_manifest_index.mark_as_loaded(chunk.id());

        self.on_store_events(&store_events);

        // We inform the stats last, since it measures e2e latency.
        // We only care about latency metrics during ingestion (adding a chunk)
        // which is why we only call it here, and not inside of `on_store_events`
        // (we need the `chunk_timestamps`).
        self.stats.on_events(chunk_timestamps, &store_events);

        Ok(store_events)
    }

    /// We call this on any changes, before returning the store events to the outsider caller.
    pub(crate) fn on_store_events(&mut self, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        self.store_size_bytes.lock().take(); // invalidate

        let mut engine = self.storage_engine.write();

        // The query cache isn't specific to the viewer. Always update it.
        engine.cache().on_events(store_events);

        if !self.enable_viewer_indexes {
            return;
        }

        let engine = engine.downgrade();

        self.rrd_manifest_index
            .on_events(engine.store(), store_events);

        // Update our internal views by notifying them of resulting [`ChunkStoreEvent`]s.
        self.time_histogram_per_timeline.on_events(
            engine.store(),
            &self.rrd_manifest_index,
            store_events,
        );
        self.rrd_manifest_index
            .entity_tree
            .on_store_additions(store_events.iter().filter_map(|e| e.to_addition()));

        let dels = store_events
            .iter()
            .filter_map(|e| e.to_deletion())
            .collect_vec();

        // It is possible for writes to trigger deletions: specifically in the case of
        // overwritten static data leading to dangling chunks.
        let entity_paths_with_deletions =
            dels.iter().map(|e| e.chunk.entity_path().clone()).collect();

        {
            re_tracing::profile_scope!("on_store_deletions");
            self.rrd_manifest_index.entity_tree.on_store_deletions(
                &engine,
                &entity_paths_with_deletions,
                &dels,
            );
        }
    }

    pub fn set_store_info(&mut self, store_info: SetStoreInfo) {
        self.set_store_info = Some(store_info);
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(
        &mut self,
        fraction_to_purge: f32,
        time_cursor: Option<(Timeline, TimeInt)>,
    ) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        assert!((0.0..=1.0).contains(&fraction_to_purge));

        let protected_chunks = self
            .rrd_manifest_index
            .chunk_prioritizer()
            .protected_chunks()
            .clone();

        let store_events = self.gc(&GarbageCollectionOptions {
            target: GarbageCollectionTarget::DropAtLeastFraction(fraction_to_purge as _),
            time_budget: DEFAULT_GC_TIME_BUDGET,

            #[expect(clippy::bool_to_int_with_if)]
            protect_latest: if self.rrd_manifest_index.has_manifest() {
                // We can redownload data, so we are free to drop anything.
                // This makes the GC faster.
                // Also, if it is important, then chunk is already in the `protected_chunks` set,
                // which is based (in part) on the chunks used in the previous frame.
                0
            } else {
                1 // We can't redownload data, so always keep the latest data point of each component
            },

            // NOTE: This will only apply if the GC is forced to fall back to row ID based collection,
            // otherwise timestamp-based collection will ignore it.
            protected_time_ranges: Default::default(),

            protected_chunks,

            furthest_from: if self.rrd_manifest_index.has_manifest() {
                // If we have an RRD manifest, it means we can download chunks on-demand.
                // So it makes sense to GC the things furthest from the current time cursor:
                time_cursor.map(|(timeline, time)| (*timeline.name(), time))
            } else {
                // If we don't have an RRD manifest, then we can't redownload data,
                // and we GC the oldest data instead.
                None
            },

            // There is no point in keeping old virtual indices for blueprint data.
            perform_deep_deletions: self.store_kind() == StoreKind::Blueprint,
        });

        if store_events.is_empty() {
            // If we weren't able to collect any data, then we need to GC the cache itself in order
            // to regain some space.
            // See <https://github.com/rerun-io/rerun/issues/7369#issuecomment-2335164098> for the
            // complete rationale.
            self.storage_engine
                .write()
                .cache()
                .purge_fraction_of_ram(fraction_to_purge);
        } else {
            self.on_store_events(&store_events);
        }

        store_events
    }

    /// The chunk store events are not handled within this function!
    #[must_use]
    pub fn gc(&self, gc_options: &GarbageCollectionOptions) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        let (store_events, stats_diff) = self.storage_engine.write().store().gc(gc_options);

        re_log::trace!(
            num_row_ids_dropped = store_events.len(),
            size_bytes_dropped = re_format::format_bytes(stats_diff.total().total_size_bytes as _),
            "purged datastore"
        );

        store_events
    }

    /// Drop all events in the given time range from the given timeline.
    ///
    /// Used to implement undo (erase the last event from the blueprint db).
    pub fn drop_time_range(
        &mut self,
        timeline: &TimelineName,
        drop_range: AbsoluteTimeRange,
    ) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        let store_events = self
            .storage_engine
            .write()
            .store()
            .drop_time_range_deep(timeline, drop_range);

        self.on_store_events(&store_events);

        store_events
    }

    /// Unconditionally drops all the data for a given [`EntityPath`] .
    ///
    /// This is _not_ recursive. Children of this entity will not be affected.
    ///
    /// To drop the entire subtree below an entity, see: [`Self::drop_entity_path_recursive`].
    pub fn drop_entity_path(&mut self, entity_path: &EntityPath) {
        re_tracing::profile_function!();

        let store_events = self
            .storage_engine
            .write()
            .store()
            .drop_entity_path(entity_path);

        self.on_store_events(&store_events);
    }

    /// Unconditionally drops all the data for a given [`EntityPath`] and all its children.
    pub fn drop_entity_path_recursive(&mut self, entity_path: &EntityPath) {
        re_tracing::profile_function!();

        let mut to_drop = vec![entity_path.clone()];

        if let Some(tree) = self.tree().subtree(entity_path) {
            tree.visit_children_recursively(|path| {
                to_drop.push(path.clone());
            });
        }

        for entity_path in to_drop {
            self.drop_entity_path(&entity_path);
        }
    }

    /// Export the contents of the current database to a sequence of messages.
    ///
    /// If `time_selection` is specified, then only data for that specific timeline over that
    /// specific time range will be accounted for.
    pub fn to_messages(
        &self,
        time_selection: Option<(TimelineName, AbsoluteTimeRangeF)>,
    ) -> impl Iterator<Item = ChunkResult<LogMsg>> + '_ {
        re_tracing::profile_function!();

        let engine = self.storage_engine.read();

        let set_store_info_msg = self
            .store_info_msg()
            .map(|msg| Ok(LogMsg::SetStoreInfo(msg.clone())));

        let data_messages = {
            let time_filter = time_selection.map(|(timeline, range)| {
                (
                    timeline,
                    AbsoluteTimeRange::new(range.min.floor(), range.max.ceil()),
                )
            });

            let mut chunks: Vec<Arc<Chunk>> = engine
                .store()
                .iter_physical_chunks()
                .filter(move |chunk| {
                    if chunk.is_static() {
                        return true; // always keep all static data
                    }

                    let Some((timeline, time_range)) = time_filter else {
                        return true; // no filter -> keep all data
                    };

                    // TODO(cmc): chunk.slice_time_selection(time_selection)
                    chunk
                        .timelines()
                        .get(&timeline)
                        .is_some_and(|time_column| time_range.intersects(time_column.time_range()))
                })
                .cloned() // refcount
                .collect();

            // Try to roughly preserve the order of the chunks
            // from how they were originally logged.
            // See https://github.com/rerun-io/rerun/issues/7175 for why.
            chunks.sort_by_key(|chunk| chunk.row_id_range().map(|(min, _)| min));

            chunks.into_iter().map(|chunk| {
                chunk
                    .to_arrow_msg()
                    .map(|msg| LogMsg::ArrowMsg(self.store_id().clone(), msg))
            })
        };

        // If this is a blueprint, make sure to include the `BlueprintActivationCommand` message.
        // We generally use `to_messages` to export a blueprint via "save". In that
        // case, we want to make the blueprint active and default when it's reloaded.
        // TODO(jleibs): Coupling this with the stored file instead of injecting seems
        // architecturally weird. Would be great if we didn't need this in `.rbl` files
        // at all.
        let blueprint_ready = if self.store_kind() == StoreKind::Blueprint {
            let activate_cmd =
                re_log_types::BlueprintActivationCommand::make_active(self.store_id().clone());

            itertools::Either::Left(std::iter::once(Ok(activate_cmd.into())))
        } else {
            itertools::Either::Right(std::iter::empty())
        };

        set_store_info_msg
            .into_iter()
            .chain(data_messages)
            .chain(blueprint_ready)
    }

    /// Make a clone of this [`EntityDb`], assigning it a new [`StoreId`].
    pub fn clone_with_new_id(&self, new_id: StoreId) -> Result<Self, Error> {
        re_tracing::profile_function!();

        let mut new_db = Self::new(new_id.clone());

        new_db.enable_viewer_indexes = self.enable_viewer_indexes;
        new_db.last_modified_at = self.last_modified_at;
        new_db.latest_row_id = self.latest_row_id;

        // We do NOT clone the `data_source`, because the reason we clone an entity db
        // is so that we can modify it, and then it would be wrong to say its from the same source.
        // Specifically: if we load a blueprint from an `.rdd`, then modify it heavily and save it,
        // it would be wrong to claim that this was the blueprint from that `.rrd`,
        // and it would confuse the user.
        // TODO(emilk): maybe we should use a special `Cloned` data source,
        // wrapping either the original source, the original StoreId, or both.

        if let Some(store_info) = self.store_info() {
            let mut new_info = store_info.clone();
            new_info.store_id = new_id;
            new_info.cloned_from = Some(self.store_id().clone());

            new_db.set_store_info(SetStoreInfo {
                row_id: *RowId::new(),
                info: new_info,
            });
        }

        let engine = self.storage_engine.read();
        for chunk in engine.store().iter_physical_chunks() {
            new_db.add_chunk(&Arc::clone(chunk))?;
        }

        Ok(new_db)
    }
}

/// ## Stats
impl EntityDb {
    /// Returns the stats for the static store of the entity and all its children, recursively.
    ///
    /// This excludes temporal data.
    pub fn subtree_stats_static(
        &self,
        engine: &StorageEngineReadGuard<'_>,
        entity_path: &EntityPath,
    ) -> ChunkStoreChunkStats {
        re_tracing::profile_function!();

        let Some(subtree) = self.tree().subtree(entity_path) else {
            return Default::default();
        };

        let mut stats = ChunkStoreChunkStats::default();
        subtree.visit_children_recursively(|path| {
            stats += engine.store().entity_stats_static(path);
        });

        stats
    }

    /// Returns the stats for the entity and all its children on the given timeline, recursively.
    ///
    /// This excludes static data.
    pub fn subtree_stats_on_timeline(
        &self,
        engine: &StorageEngineReadGuard<'_>,
        entity_path: &EntityPath,
        timeline: &TimelineName,
    ) -> ChunkStoreChunkStats {
        re_tracing::profile_function!();

        let Some(subtree) = self.tree().subtree(entity_path) else {
            return Default::default();
        };

        let mut stats = ChunkStoreChunkStats::default();
        subtree.visit_children_recursively(|path| {
            stats += engine.store().entity_stats_on_timeline(path, timeline);
        });

        stats
    }

    /// Returns true if an entity or any of its children have any data on the given timeline.
    ///
    /// This includes static data.
    pub fn subtree_has_data_on_timeline(
        &self,
        engine: &StorageEngineReadGuard<'_>,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> bool {
        re_tracing::profile_function!();

        let Some(subtree) = self.tree().subtree(entity_path) else {
            return false;
        };

        subtree
            .find_first_child_recursive(|path| {
                self.rrd_manifest_index
                    .entity_has_data_on_timeline(path, timeline)
                    || engine.store().entity_has_data_on_timeline(timeline, path)
            })
            .is_some()
    }

    /// Returns true if an entity or any of its children have any temporal data on the given timeline.
    ///
    /// This ignores static data.
    pub fn subtree_has_temporal_data_on_timeline(
        &self,
        engine: &StorageEngineReadGuard<'_>,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> bool {
        re_tracing::profile_function!();

        let Some(subtree) = self.tree().subtree(entity_path) else {
            return false;
        };

        subtree
            .find_first_child_recursive(|path| {
                self.rrd_manifest_index
                    .entity_has_temporal_data_on_timeline(path, timeline)
                    || engine
                        .store()
                        .entity_has_temporal_data_on_timeline(timeline, path)
            })
            .is_some()
    }

    /// Returns true if an entity has any temporal data on the given timeline.
    ///
    /// This ignores static data.
    pub fn entity_has_temporal_data_on_timeline(
        &self,
        engine: &StorageEngineReadGuard<'_>,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> bool {
        re_tracing::profile_function!();

        self.rrd_manifest_index
            .entity_has_temporal_data_on_timeline(entity_path, timeline)
            || engine
                .store()
                .entity_has_temporal_data_on_timeline(timeline, entity_path)
    }
}

impl re_byte_size::SizeBytes for EntityDb {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            store_id,
            enable_viewer_indexes,
            data_source: _,
            rrd_manifest_index,
            set_store_info,
            last_modified_at: _,
            latest_row_id: _,
            entity_paths,
            entity_path_from_hash,
            time_histogram_per_timeline,
            storage_engine,
            store_size_bytes,
            stats: _,
        } = self;

        let storage_engine = storage_engine.read();

        let store_size_bytes = {
            // Calculate lazily
            *store_size_bytes
                .lock()
                .get_or_insert_with(|| storage_engine.store().heap_size_bytes())
        };

        let storage_engine_size = storage_engine.cache().heap_size_bytes() + store_size_bytes;

        store_id.heap_size_bytes()
            + enable_viewer_indexes.heap_size_bytes()
            + rrd_manifest_index.heap_size_bytes()
            + set_store_info.heap_size_bytes()
            + entity_paths.heap_size_bytes()
            + entity_path_from_hash.heap_size_bytes()
            + time_histogram_per_timeline.heap_size_bytes()
            + storage_engine_size
    }
}

impl MemUsageTreeCapture for EntityDb {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            rrd_manifest_index,
            time_histogram_per_timeline,
            storage_engine,
            entity_paths,
            entity_path_from_hash,

            // Small:
            store_id: _,
            enable_viewer_indexes: _,
            data_source: _,
            set_store_info: _,
            last_modified_at: _,
            latest_row_id: _,
            store_size_bytes: _,
            stats: _,
        } = self;

        let mut node = MemUsageNode::new();

        node.add(
            "chunk_store",
            storage_engine.read().capture_mem_usage_tree(),
        );

        node.add(
            "entity_paths",
            MemUsageTree::Bytes(entity_paths.total_size_bytes()),
        );

        node.add(
            "entity_path_from_hash",
            MemUsageTree::Bytes(entity_path_from_hash.total_size_bytes()),
        );

        node.add(
            "time_histogram_per_timeline",
            time_histogram_per_timeline.capture_mem_usage_tree(),
        );
        node.add(
            "rrd_manifest_index",
            rrd_manifest_index.capture_mem_usage_tree(),
        );

        node.into_tree()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreId, TimePoint, Timeline};

    use super::*;

    #[test]
    fn format_with_components() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut db = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));

        let timeline_frame = Timeline::new_sequence("frame");

        // Add some test data
        {
            let row_id = RowId::new();
            let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
            let point = MyPoint::new(1.0, 2.0);
            let chunk = Chunk::builder("parent/child1/grandchild")
                .with_component_batches(
                    row_id,
                    timepoint,
                    [(MyPoints::descriptor_points(), &[point] as _)],
                )
                .build()?;

            db.add_chunk(&Arc::new(chunk))?;
        }

        assert_eq!(
            db.format_with_components(),
            "/parent\n  /parent/child1\n    /parent/child1/grandchild\n      example.MyPoint: Struct[2]\n"
        );

        Ok(())
    }
}
