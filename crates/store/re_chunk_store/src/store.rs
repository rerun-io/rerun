use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use arrow::datatypes::DataType as ArrowDataType;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, TimelineName};
use re_log_types::{EntityPath, StoreId, TimeInt, TimeType};
use re_types_core::{ComponentDescriptor, ComponentType};

use crate::{ChunkStoreChunkStats, ChunkStoreError, ChunkStoreResult};

// ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkStoreConfig {
    /// If `true` (the default), the store will emit events when its contents are modified in
    /// any way (insertion, GC), that can be subscribed to.
    ///
    /// Leaving this disabled can lead to major performance improvements on the ingestion path
    /// in some workloads, provided that the subscribers aren't needed (e.g. headless mode).
    pub enable_changelog: bool,

    /// What is the threshold, in bytes, after which a [`Chunk`] cannot be compacted any further?
    ///
    /// This is a multi-dimensional trade-off:
    /// * Larger chunks lead to less fixed overhead introduced by metadata, indices and such. Good.
    /// * Larger chunks lead to slower query execution on some unhappy paths. Bad.
    /// * Larger chunks lead to slower and slower compaction as chunks grow larger. Bad.
    /// * Larger chunks lead to coarser garbage collection. Good or bad depending on use case.
    /// * Larger chunks lead to less precision in e.g. the time panel. Bad.
    ///
    /// Empirical testing shows that the space overhead gains rapidly diminish beyond ~1000 rows,
    /// which is the default row threshold.
    /// The default byte threshold is set to 8MiB, which is a reasonable unit of work when e.g.
    /// sending chunks over the network.
    pub chunk_max_bytes: u64,

    /// What is the threshold, in rows, after which a [`Chunk`] cannot be compacted any further?
    ///
    /// This specifically applies to time-sorted chunks.
    /// See also [`ChunkStoreConfig::chunk_max_rows_if_unsorted`].
    ///
    /// This is a multi-dimensional trade-off:
    /// * Larger chunks lead to less fixed overhead introduced by metadata, indices and such. Good.
    /// * Larger chunks lead to slower query execution on some unhappy paths. Bad.
    /// * Larger chunks lead to slower and slower compaction as chunks grow larger. Bad.
    /// * Larger chunks lead to coarser garbage collection. Good or bad depending on use case.
    /// * Larger chunks lead to less precision in e.g. the time panel. Bad.
    ///
    /// Empirical testing shows that the space overhead gains rapidly diminish beyond ~1000 rows,
    /// which is the default row threshold.
    /// The default byte threshold is set to 8MiB, which is a reasonable unit of work when e.g.
    /// sending chunks over the network.
    pub chunk_max_rows: u64,

    /// What is the threshold, in rows, after which a [`Chunk`] cannot be compacted any further?
    ///
    /// This specifically applies to _non_ time-sorted chunks.
    /// See also [`ChunkStoreConfig::chunk_max_rows`].
    ///
    /// This is a multi-dimensional trade-off:
    /// * Larger chunks lead to less fixed overhead introduced by metadata, indices and such. Good.
    /// * Larger chunks lead to slower query execution on some unhappy paths. Bad.
    /// * Larger chunks lead to slower and slower compaction as chunks grow larger. Bad.
    /// * Larger chunks lead to coarser garbage collection. Good or bad depending on use case.
    /// * Larger chunks lead to less precision in e.g. the time panel. Bad.
    ///
    /// Empirical testing shows that the space overhead gains rapidly diminish beyond ~1000 rows,
    /// which is the default row threshold.
    /// The default byte threshold is set to 8MiB, which is a reasonable unit of work when e.g.
    /// sending chunks over the network.
    pub chunk_max_rows_if_unsorted: u64,
    //
    // TODO(cmc): It could make sense to have time-range-based thresholds in here, since the time
    // range covered by a chunk has direct effects on A) the complexity of backward walks and
    // B) in downstream subscribers (e.g. the precision of the time panel).
    //
    // In practice this is highly recording-dependent, and would require either to make it
    // user-configurable per-recording, or use heuristics to compute it on the fly.
    //
    // The added complexity just isn't worth it at the moment.
    // Maybe at some point.
}

impl Default for ChunkStoreConfig {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ChunkStoreConfig {
    /// Default configuration, applicable to most use cases, according to empirical testing.
    pub const DEFAULT: Self = Self {
        enable_changelog: true,

        // This gives us 96 bytes per row (assuming a default limit of 4096 rows), which is enough to
        // fit a couple scalar columns, a RowId column, a handful of timeline columns, all the
        // necessary offsets, etc.
        //
        // A few megabytes turned out to be way too costly to concatenate in real-time in the
        // Viewer (see <https://github.com/rerun-io/rerun/issues/7222>).
        chunk_max_bytes: 12 * 8 * 4096,

        // Empirical testing shows that 4096 is the threshold after which we really start to get
        // dimishing returns space and compute wise.
        chunk_max_rows: 4096,

        chunk_max_rows_if_unsorted: 1024,
    };

    /// [`Self::DEFAULT`], but with compaction entirely disabled.
    pub const COMPACTION_DISABLED: Self = Self {
        chunk_max_bytes: 0,
        chunk_max_rows: 0,
        chunk_max_rows_if_unsorted: 0,
        ..Self::DEFAULT
    };

    /// [`Self::DEFAULT`], but with changelog disabled.
    pub const CHANGELOG_DISABLED: Self = Self {
        enable_changelog: false,
        ..Self::DEFAULT
    };

    /// All features disabled.
    pub const ALL_DISABLED: Self = Self {
        enable_changelog: false,
        chunk_max_bytes: 0,
        chunk_max_rows: 0,
        chunk_max_rows_if_unsorted: 0,
    };

    /// Environment variable to configure [`Self::enable_changelog`].
    pub const ENV_STORE_ENABLE_CHANGELOG: &'static str = "RERUN_STORE_ENABLE_CHANGELOG";

    /// Environment variable to configure [`Self::chunk_max_bytes`].
    pub const ENV_CHUNK_MAX_BYTES: &'static str = "RERUN_CHUNK_MAX_BYTES";

    /// Environment variable to configure [`Self::chunk_max_rows`].
    pub const ENV_CHUNK_MAX_ROWS: &'static str = "RERUN_CHUNK_MAX_ROWS";

    /// Environment variable to configure [`Self::chunk_max_rows_if_unsorted`].
    //
    // NOTE: Shared with the same env-var on the batcher side, for consistency.
    pub const ENV_CHUNK_MAX_ROWS_IF_UNSORTED: &'static str = "RERUN_CHUNK_MAX_ROWS_IF_UNSORTED";

    /// Creates a new `ChunkStoreConfig` using the default values, optionally overridden
    /// through the environment.
    ///
    /// See [`Self::apply_env`].
    #[inline]
    pub fn from_env() -> ChunkStoreResult<Self> {
        Self::default().apply_env()
    }

    /// Returns a copy of `self`, overriding existing fields with values from the environment if
    /// they are present.
    ///
    /// See [`Self::ENV_STORE_ENABLE_CHANGELOG`], [`Self::ENV_CHUNK_MAX_BYTES`], [`Self::ENV_CHUNK_MAX_ROWS`]
    /// and [`Self::ENV_CHUNK_MAX_ROWS_IF_UNSORTED`].
    pub fn apply_env(&self) -> ChunkStoreResult<Self> {
        let mut new = self.clone();

        if let Ok(s) = std::env::var(Self::ENV_STORE_ENABLE_CHANGELOG) {
            new.enable_changelog = s.parse().map_err(|err| ChunkStoreError::ParseConfig {
                name: Self::ENV_STORE_ENABLE_CHANGELOG,
                value: s.clone(),
                err: Box::new(err),
            })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_CHUNK_MAX_BYTES) {
            new.chunk_max_bytes = s.parse().map_err(|err| ChunkStoreError::ParseConfig {
                name: Self::ENV_CHUNK_MAX_BYTES,
                value: s.clone(),
                err: Box::new(err),
            })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_CHUNK_MAX_ROWS) {
            new.chunk_max_rows = s.parse().map_err(|err| ChunkStoreError::ParseConfig {
                name: Self::ENV_CHUNK_MAX_ROWS,
                value: s.clone(),
                err: Box::new(err),
            })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_CHUNK_MAX_ROWS_IF_UNSORTED) {
            new.chunk_max_rows_if_unsorted =
                s.parse().map_err(|err| ChunkStoreError::ParseConfig {
                    name: Self::ENV_CHUNK_MAX_ROWS_IF_UNSORTED,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        Ok(new)
    }
}

#[test]
fn chunk_store_config() {
    // Detect breaking changes in our environment variables.

    // SAFETY: it's a test
    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("RERUN_STORE_ENABLE_CHANGELOG", "false");
        std::env::set_var("RERUN_CHUNK_MAX_BYTES", "42");
        std::env::set_var("RERUN_CHUNK_MAX_ROWS", "666");
        std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "999");
    };

    let config = ChunkStoreConfig::from_env().unwrap();

    let expected = ChunkStoreConfig {
        enable_changelog: false,
        chunk_max_bytes: 42,
        chunk_max_rows: 666,
        chunk_max_rows_if_unsorted: 999,
    };

    assert_eq!(expected, config);
}

// ---

pub type ChunkIdSet = BTreeSet<ChunkId>;

#[derive(Debug, Default, Clone)]
pub struct ChunkIdSetPerTime {
    /// Keeps track of the longest interval being currently stored in the two maps below.
    ///
    /// This is used to bound the backwards linear walk when looking for overlapping chunks in
    /// latest-at queries.
    ///
    /// This is purely additive: this value is never decremented for any reason, whether it's GC,
    /// chunk splitting, or whatever else.
    ///
    /// See [`ChunkStore::latest_at`] implementation comments for more details.
    pub(crate) max_interval_length: u64,

    /// *Both physical & virtual* [`ChunkId`]s organized by their _most specific_ start time.
    ///
    /// What "most specific" means depends on the context in which the [`ChunkIdSetPerTime`]
    /// was instantiated, e.g.:
    /// * For an `(entity, timeline, component)` index, that would be the first timestamp at which this
    ///   [`Chunk`] contains data for this particular component on this particular timeline (see
    ///   [`Chunk::time_range_per_component`]).
    /// * For an `(entity, timeline)` index, that would be the first timestamp at which this [`Chunk`]
    ///   contains data for any component on this particular timeline (see [`re_chunk::TimeColumn::time_range`]).
    ///
    /// This index includes virtual/offloaded chunks, and therefore is purely additive: garbage collection
    /// will never remove values from this set.
    pub(crate) per_start_time: BTreeMap<TimeInt, ChunkIdSet>,

    /// *Both physical & virtual* [`ChunkId`]s organized by their _most specific_ end time.
    ///
    /// What "most specific" means depends on the context in which the [`ChunkIdSetPerTime`]
    /// was instantiated, e.g.:
    /// * For an `(entity, timeline, component)` index, that would be the last timestamp at which this
    ///   [`Chunk`] contains data for this particular component on this particular timeline (see
    ///   [`Chunk::time_range_per_component`]).
    /// * For an `(entity, timeline)` index, that would be the last timestamp at which this [`Chunk`]
    ///   contains data for any component on this particular timeline (see [`re_chunk::TimeColumn::time_range`]).
    ///
    /// This index includes virtual/offloaded chunks, and therefore is purely additive: garbage collection
    /// will never remove values from this set.
    pub(crate) per_end_time: BTreeMap<TimeInt, ChunkIdSet>,
}

pub type ChunkIdSetPerTimePerComponent = IntMap<ComponentIdentifier, ChunkIdSetPerTime>;

pub type ChunkIdSetPerTimePerComponentPerTimeline =
    IntMap<TimelineName, ChunkIdSetPerTimePerComponent>;

pub type ChunkIdSetPerTimePerComponentPerTimelinePerEntity =
    IntMap<EntityPath, ChunkIdSetPerTimePerComponentPerTimeline>;

pub type ChunkIdPerComponent = IntMap<ComponentIdentifier, ChunkId>;

pub type ChunkIdPerComponentPerEntity = IntMap<EntityPath, ChunkIdPerComponent>;

pub type ChunkIdSetPerTimePerTimeline = IntMap<TimelineName, ChunkIdSetPerTime>;

pub type ChunkIdSetPerTimePerTimelinePerEntity = IntMap<EntityPath, ChunkIdSetPerTimePerTimeline>;

// ---

#[derive(Debug, Clone)]
pub struct ColumnMetadata {
    /// Whether this column represents static data.
    pub is_static: bool,

    /// Whether this column represents a `Clear`-related component.
    ///
    /// `Clear`: [`re_types_core::archetypes::Clear`]
    pub is_tombstone: bool,

    /// Whether this column contains either no data or only contains null and/or empty values (`[]`).
    pub is_semantically_empty: bool,
}

/// Internal state that needs to be maintained in order to compute [`ColumnMetadata`].
#[derive(Debug, Clone)]
pub struct ColumnMetadataState {
    /// Whether this column contains either no data or only contains null and/or empty values (`[]`).
    ///
    /// This is purely additive: once false, it will always be false. Even in case of garbage
    /// collection.
    pub is_semantically_empty: bool,
}

/// Incremented on each edit.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChunkStoreGeneration {
    insert_id: u64,
    gc_id: u64,
}

/// A ref-counted, inner-mutable handle to a [`ChunkStore`].
///
/// Cheap to clone.
///
/// It is possible to grab the lock behind this handle while _maintaining a static lifetime_, see:
/// * [`ChunkStoreHandle::read_arc`]
/// * [`ChunkStoreHandle::write_arc`]
#[derive(Clone)]
pub struct ChunkStoreHandle(Arc<parking_lot::RwLock<ChunkStore>>);

impl std::fmt::Display for ChunkStoreHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0.read()))
    }
}

impl ChunkStoreHandle {
    #[inline]
    pub fn new(store: ChunkStore) -> Self {
        Self(Arc::new(parking_lot::RwLock::new(store)))
    }

    #[inline]
    pub fn into_inner(self) -> Arc<parking_lot::RwLock<ChunkStore>> {
        self.0
    }
}

impl ChunkStoreHandle {
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, ChunkStore> {
        self.0.read_recursive()
    }

    #[inline]
    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, ChunkStore>> {
        self.0.try_read_recursive()
    }

    #[inline]
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, ChunkStore> {
        self.0.write()
    }

    #[inline]
    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, ChunkStore>> {
        self.0.try_write()
    }

    #[inline]
    pub fn read_arc(&self) -> parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, ChunkStore> {
        parking_lot::RwLock::read_arc_recursive(&self.0)
    }

    #[inline]
    pub fn try_read_arc(
        &self,
    ) -> Option<parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, ChunkStore>> {
        parking_lot::RwLock::try_read_recursive_arc(&self.0)
    }

    #[inline]
    pub fn write_arc(
        &self,
    ) -> parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, ChunkStore> {
        parking_lot::RwLock::write_arc(&self.0)
    }

    #[inline]
    pub fn try_write_arc(
        &self,
    ) -> Option<parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, ChunkStore>> {
        parking_lot::RwLock::try_write_arc(&self.0)
    }
}

/// A complete chunk store: covers all timelines, all entities, everything.
///
/// The chunk store _always_ works at the chunk level, whether it is for write & read queries or
/// garbage collection. It is completely oblivious to individual rows.
///
/// Use the `Display` implementation for a detailed view of the internals.
#[derive(Debug)]
pub struct ChunkStore {
    pub(crate) id: StoreId,

    /// The configuration of the chunk store (e.g. compaction settings).
    pub(crate) config: ChunkStoreConfig,

    /// Keeps track of the _latest_ datatype for each time column.
    ///
    /// This index is purely additive: it is never affected by garbage collection in any way.
    ///
    /// See also [`Self::time_column_type`].
    pub(crate) time_type_registry: IntMap<TimelineName, TimeType>,

    /// Keeps track of the _latest_ datatype information for all component types that have been written
    /// to the store so far.
    ///
    /// This index is purely additive: it is never affected by garbage collection in any way.
    ///
    /// See also [`Self::lookup_datatype`].
    //
    // TODO(cmc): this would become fairly problematic in a world where each chunk can use a
    // different datatype for a given component.
    pub(crate) type_registry: IntMap<ComponentType, ArrowDataType>,

    // TODO(grtlr): Can we slim this map down by getting rid of `ColumnIdentifier`-level here?
    pub(crate) per_column_metadata: IntMap<
        EntityPath,
        IntMap<ComponentIdentifier, (ComponentDescriptor, ColumnMetadataState, ArrowDataType)>,
    >,

    /// All the *physical* chunks currently loaded in the store, mapped by their respective IDs.
    ///
    /// Physical chunks are chunks that are actively loaded into the store's volatile memory.
    ///
    /// During garbage collection, physical chunks are offloaded from memory and become virtual
    /// chunks instead. At the same time, their IDs are removed from this set, which is how we
    /// distinguish virtual from physical chunks.
    ///
    /// Virtual chunks are still indexed by the store, but querying for them will not yield any data,
    /// just hints that some data is missing and must first be re-inserted by the caller.
    pub(crate) chunks_per_chunk_id: BTreeMap<ChunkId, Arc<Chunk>>,

    /// All *physical* [`ChunkId`]s currently in the store, indexed by the smallest [`RowId`] in
    /// each of them.
    ///
    /// This is effectively all chunks in global data order. Used for garbage collection.
    ///
    /// During garbage collection, physical chunks are offloaded from memory and become virtual
    /// chunks instead. At the same time, their IDs are removed from this set, which is how we
    /// distinguish virtual from physical chunks.
    pub(crate) chunk_ids_per_min_row_id: BTreeMap<RowId, ChunkId>,

    /// All *physical & virtual* temporal [`ChunkId`]s for all entities on all timelines, further
    /// indexed by [`ComponentIdentifier`].
    ///
    /// This index is purely additive: it is never affected by garbage collection in any way.
    /// This implies that the chunk IDs present in this set might be either physical/loaded or
    /// virtual/offloaded.
    /// When leveraging this index, make sure you understand whether you expect loaded chunks,
    /// unloaded chunks, or both. Leverage [`Self::chunks_per_chunk_id`] to know which is which.
    ///
    /// See also:
    /// * [`Self::temporal_chunk_ids_per_entity`].
    /// * [`Self::static_chunk_ids_per_entity`].
    pub(crate) temporal_chunk_ids_per_entity_per_component:
        ChunkIdSetPerTimePerComponentPerTimelinePerEntity,

    /// All *physical & virtual* temporal [`ChunkId`]s for all entities on all timelines, without the
    /// [`ComponentType`] index.
    ///
    /// This index is purely additive: it is never affected by garbage collection in any way.
    /// This implies that the chunk IDs present in this set might be either physical/loaded or
    /// virtual/offloaded.
    /// When leveraging this index, make sure you understand whether you expect loaded chunks,
    /// unloaded chunks, or both. Leverage [`Self::chunks_per_chunk_id`] to know which is which.
    ///
    /// See also:
    /// * [`Self::temporal_chunk_ids_per_entity_per_component`].
    /// * [`Self::static_chunk_ids_per_entity`].
    pub(crate) temporal_chunk_ids_per_entity: ChunkIdSetPerTimePerTimelinePerEntity,

    /// Accumulated size statitistics for all *physical* temporal [`Chunk`]s currently present in the store.
    ///
    /// This is too costly to be computed from scratch every frame, and therefore materialized here.
    ///
    /// *This exclusively covers physical/loaded chunks*. During GC, these statistics are decremented
    /// as you'd expect.
    pub(crate) temporal_physical_chunks_stats: ChunkStoreChunkStats,

    /// Static data. Never garbage collected.
    ///
    /// Static data unconditionally shadows temporal data at query time.
    ///
    /// Existing temporal will not be removed. Events won't be fired.
    pub(crate) static_chunk_ids_per_entity: ChunkIdPerComponentPerEntity,

    /// Accumulated size statitistics for all *physical* static [`Chunk`]s currently present in the store.
    ///
    /// This is too costly to be computed from scratch every frame, and is therefore materialized here.
    pub(crate) static_chunks_stats: ChunkStoreChunkStats,

    /// Monotonically increasing ID for insertions.
    pub(crate) insert_id: u64,

    /// Monotonically increasing ID for GCs.
    pub(crate) gc_id: u64,

    /// Monotonically increasing ID for store events.
    pub(crate) event_id: AtomicU64,
}

impl Drop for ChunkStore {
    fn drop(&mut self) {
        // First and foremost, notify per-store subscribers that an entire store was just dropped,
        // and therefore they can just drop entire chunks of their own state.
        Self::drop_per_store_subscribers(&self.id());

        if self.config.enable_changelog {
            // Then, if the changelog is enabled, trigger a full GC: this will notify all remaining
            // subscribers of all the chunks that were dropped by dropping the store itself.
            _ = self.gc(&crate::GarbageCollectionOptions::gc_everything());
        }
    }
}

impl Clone for ChunkStore {
    #[inline]
    fn clone(&self) -> Self {
        re_tracing::profile_function!();
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            time_type_registry: self.time_type_registry.clone(),
            type_registry: self.type_registry.clone(),
            per_column_metadata: self.per_column_metadata.clone(),
            chunks_per_chunk_id: self.chunks_per_chunk_id.clone(),
            chunk_ids_per_min_row_id: self.chunk_ids_per_min_row_id.clone(),
            temporal_chunk_ids_per_entity_per_component: self
                .temporal_chunk_ids_per_entity_per_component
                .clone(),
            temporal_chunk_ids_per_entity: self.temporal_chunk_ids_per_entity.clone(),
            temporal_physical_chunks_stats: self.temporal_physical_chunks_stats,
            static_chunk_ids_per_entity: self.static_chunk_ids_per_entity.clone(),
            static_chunks_stats: self.static_chunks_stats,
            insert_id: Default::default(),
            gc_id: Default::default(),
            event_id: Default::default(),
        }
    }
}

impl std::fmt::Display for ChunkStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            config,
            time_type_registry: _,
            type_registry: _,
            per_column_metadata: _,
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component: _,
            temporal_chunk_ids_per_entity: _,
            temporal_physical_chunks_stats,
            static_chunk_ids_per_entity: _,
            static_chunks_stats,
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        f.write_str("ChunkStore {\n")?;

        f.write_str(&indent::indent_all_by(4, format!("id: {id:?}\n")))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        f.write_str(&indent::indent_all_by(4, "stats: {\n"))?;
        f.write_str(&indent::indent_all_by(
            8,
            format!("{}", *static_chunks_stats + *temporal_physical_chunks_stats),
        ))?;
        f.write_str(&indent::indent_all_by(4, "}\n"))?;

        f.write_str(&indent::indent_all_by(4, "chunks: [\n"))?;
        for chunk_id in chunk_ids_per_min_row_id.values() {
            if let Some(chunk) = chunks_per_chunk_id.get(chunk_id) {
                if let Some(width) = f.width() {
                    let chunk_width = width.saturating_sub(8);
                    f.write_str(&indent::indent_all_by(8, format!("{chunk:chunk_width$}\n")))?;
                } else {
                    f.write_str(&indent::indent_all_by(8, format!("{chunk}\n")))?;
                }
            } else {
                f.write_str(&indent::indent_all_by(8, "<not_found>\n"))?;
            }
        }
        f.write_str(&indent::indent_all_by(4, "]\n"))?;

        f.write_str("}")?;

        Ok(())
    }
}

// ---

impl ChunkStore {
    /// Instantiate a new empty `ChunkStore` with the given [`ChunkStoreConfig`].
    ///
    /// See also:
    /// * [`ChunkStore::new`]
    /// * [`ChunkStore::from_rrd_filepath`]
    #[inline]
    pub fn new(id: StoreId, config: ChunkStoreConfig) -> Self {
        Self {
            id,
            config,
            time_type_registry: Default::default(),
            type_registry: Default::default(),
            per_column_metadata: Default::default(),
            chunk_ids_per_min_row_id: Default::default(),
            chunks_per_chunk_id: Default::default(),
            temporal_chunk_ids_per_entity_per_component: Default::default(),
            temporal_chunk_ids_per_entity: Default::default(),
            temporal_physical_chunks_stats: Default::default(),
            static_chunk_ids_per_entity: Default::default(),
            static_chunks_stats: Default::default(),
            insert_id: 0,
            gc_id: 0,
            event_id: AtomicU64::new(0),
        }
    }

    /// Instantiate a new empty `ChunkStore` with the given [`ChunkStoreConfig`].
    ///
    /// Pre-wraps the result in a [`ChunkStoreHandle`].
    ///
    /// See also:
    /// * [`ChunkStore::from_rrd_filepath`]
    #[inline]
    pub fn new_handle(id: StoreId, config: ChunkStoreConfig) -> ChunkStoreHandle {
        ChunkStoreHandle::new(Self::new(id, config))
    }

    #[inline]
    pub fn id(&self) -> StoreId {
        self.id.clone()
    }

    /// Return the current [`ChunkStoreGeneration`]. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    #[inline]
    pub fn generation(&self) -> ChunkStoreGeneration {
        ChunkStoreGeneration {
            insert_id: self.insert_id,
            gc_id: self.gc_id,
        }
    }

    /// See [`ChunkStoreConfig`] for more information about configuration.
    #[inline]
    pub fn config(&self) -> &ChunkStoreConfig {
        &self.config
    }

    /// Iterate over all chunks in the store, in ascending [`ChunkId`] order.
    #[inline]
    pub fn iter_chunks(&self) -> impl Iterator<Item = &Arc<Chunk>> + '_ {
        self.chunks_per_chunk_id.values()
    }

    /// Get a chunk based on its ID.
    #[inline]
    pub fn chunk(&self, id: &ChunkId) -> Option<&Arc<Chunk>> {
        self.chunks_per_chunk_id.get(id)
    }

    /// Get the number of chunks.
    #[inline]
    pub fn num_chunks(&self) -> usize {
        self.chunks_per_chunk_id.len()
    }

    /// Lookup the _latest_ [`TimeType`] used by a specific [`TimelineName`].
    #[inline]
    pub fn time_column_type(&self, timeline_name: &TimelineName) -> Option<TimeType> {
        self.time_type_registry.get(timeline_name).copied()
    }

    /// Lookup the _latest_ arrow [`ArrowDataType`] used by a specific [`re_types_core::Component`].
    #[inline]
    pub fn lookup_datatype(&self, component_type: &ComponentType) -> Option<ArrowDataType> {
        self.type_registry.get(component_type).cloned()
    }

    /// Lookup the [`ColumnMetadata`] for a specific [`EntityPath`] and [`re_types_core::Component`].
    pub fn lookup_column_metadata(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<ColumnMetadata> {
        let ColumnMetadataState {
            is_semantically_empty,
        } = self
            .per_column_metadata
            .get(entity_path)
            .and_then(|per_identifier| per_identifier.get(&component))
            .map(|(_, metadata_state, _)| metadata_state)?;

        let is_static = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .is_some_and(|per_component| per_component.get(&component).is_some());

        use re_types_core::Archetype as _;
        let is_tombstone = re_types_core::archetypes::Clear::all_components()
            .iter()
            .any(|descr| descr.component == component);

        Some(ColumnMetadata {
            is_static,
            is_tombstone,
            is_semantically_empty: *is_semantically_empty,
        })
    }
}

// ---

impl ChunkStore {
    /// Instantiate a new `ChunkStore` with the given [`ChunkStoreConfig`].
    ///
    /// The stores will be prefilled with the data at the specified path.
    ///
    /// See also:
    /// * [`ChunkStore::new`]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_rrd_filepath(
        store_config: &ChunkStoreConfig,
        path_to_rrd: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<BTreeMap<StoreId, Self>> {
        let path_to_rrd = path_to_rrd.as_ref();

        re_tracing::profile_function!(path_to_rrd.to_string_lossy());

        use anyhow::Context as _;

        let mut stores = BTreeMap::new();

        let rrd_file = std::fs::File::open(path_to_rrd)
            .with_context(|| format!("couldn't open {path_to_rrd:?}"))?;

        let decoder = re_log_encoding::Decoder::decode_eager(std::io::BufReader::new(rrd_file))
            .with_context(|| format!("couldn't decode {path_to_rrd:?}"))?;

        // TODO(cmc): offload the decoding to a background thread.
        for res in decoder {
            let msg = res.with_context(|| format!("couldn't decode message {path_to_rrd:?}"))?;
            match msg {
                re_log_types::LogMsg::SetStoreInfo(info) => {
                    stores.entry(info.info.store_id.clone()).or_insert_with(|| {
                        Self::new(info.info.store_id.clone(), store_config.clone())
                    });
                }

                re_log_types::LogMsg::ArrowMsg(store_id, msg) => {
                    let Some(store) = stores.get_mut(&store_id) else {
                        anyhow::bail!("unknown store ID: {store_id:?}");
                    };

                    let chunk = Chunk::from_arrow_msg(&msg)
                        .with_context(|| format!("couldn't decode chunk {path_to_rrd:?}"))?;

                    store
                        .insert_chunk(&Arc::new(chunk))
                        .with_context(|| format!("couldn't insert chunk {path_to_rrd:?}"))?;
                }

                re_log_types::LogMsg::BlueprintActivationCommand(_) => {}
            }
        }

        Ok(stores)
    }

    /// Instantiate a new `ChunkStore` with the given [`ChunkStoreConfig`].
    ///
    /// The stores will be prefilled with the data in the given `log_msgs`.
    ///
    /// See also:
    /// * [`ChunkStore::new`]
    pub fn from_log_msgs(
        store_config: &ChunkStoreConfig,
        log_msgs: impl IntoIterator<Item = re_log_types::LogMsg>,
    ) -> anyhow::Result<BTreeMap<StoreId, Self>> {
        re_tracing::profile_function!();

        use anyhow::Context as _;

        let mut stores = BTreeMap::new();

        // TODO(cmc): offload the decoding to a background thread.
        let log_msgs = log_msgs.into_iter();
        for msg in log_msgs {
            match msg {
                re_log_types::LogMsg::SetStoreInfo(info) => {
                    stores.entry(info.info.store_id.clone()).or_insert_with(|| {
                        Self::new(info.info.store_id.clone(), store_config.clone())
                    });
                }

                re_log_types::LogMsg::ArrowMsg(store_id, msg) => {
                    let Some(store) = stores.get_mut(&store_id) else {
                        anyhow::bail!("unknown store ID: {store_id:?}");
                    };

                    let chunk = Chunk::from_arrow_msg(&msg)
                        .with_context(|| "couldn't decode chunk".to_owned())?;

                    store
                        .insert_chunk(&Arc::new(chunk))
                        .with_context(|| "couldn't insert chunk".to_owned())?;
                }

                re_log_types::LogMsg::BlueprintActivationCommand(_) => {}
            }
        }

        Ok(stores)
    }

    /// Instantiate a new `ChunkStore` with the given [`ChunkStoreConfig`].
    ///
    /// Wraps the results in [`ChunkStoreHandle`]s.
    ///
    /// The stores will be prefilled with the data at the specified path.
    ///
    /// See also:
    /// * [`ChunkStore::new_handle`]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn handle_from_rrd_filepath(
        store_config: &ChunkStoreConfig,
        path_to_rrd: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<BTreeMap<StoreId, ChunkStoreHandle>> {
        Ok(Self::from_rrd_filepath(store_config, path_to_rrd)?
            .into_iter()
            .map(|(store_id, store)| (store_id, ChunkStoreHandle::new(store)))
            .collect())
    }
}
