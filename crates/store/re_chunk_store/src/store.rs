use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use arrow2::datatypes::DataType as ArrowDataType;
use nohash_hasher::IntMap;

use re_chunk::{Chunk, ChunkId, RowId};
use re_log_types::{EntityPath, StoreId, TimeInt, Timeline};
use re_types_core::ComponentName;

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

        // Empirical testing shows that 4MiB is good middle-ground, big tensors and buffers can
        // become a bit too costly to concatenate beyond that.
        chunk_max_bytes: 4 * 1024 * 1024,

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
    std::env::set_var("RERUN_STORE_ENABLE_CHANGELOG", "false");
    std::env::set_var("RERUN_CHUNK_MAX_BYTES", "42");
    std::env::set_var("RERUN_CHUNK_MAX_ROWS", "666");
    std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "999");

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

#[derive(Default, Debug, Clone)]
pub struct ChunkIdSetPerTime {
    /// Keeps track of the longest interval being currently stored in the two maps below.
    ///
    /// This is used to bound the backwards linear walk when looking for overlapping chunks in
    /// latest-at queries.
    ///
    /// See [`ChunkStore::latest_at`] implementation comments for more details.
    pub(crate) max_interval_length: u64,

    /// [`ChunkId`]s organized by their _most specific_ start time.
    ///
    /// What "most specific" means depends on the context in which the [`ChunkIdSetPerTime`]
    /// was instantiated, e.g.:
    /// * For an `(entity, timeline, component)` index, that would be the first timestamp at which this
    ///   [`Chunk`] contains data for this particular component on this particular timeline (see
    ///   [`Chunk::time_range_per_component`]).
    /// * For an `(entity, timeline)` index, that would be the first timestamp at which this [`Chunk`]
    ///   contains data for any component on this particular timeline (see [`re_chunk::TimeColumn::time_range`]).
    pub(crate) per_start_time: BTreeMap<TimeInt, ChunkIdSet>,

    /// [`ChunkId`]s organized by their _most specific_ end time.
    ///
    /// What "most specific" means depends on the context in which the [`ChunkIdSetPerTime`]
    /// was instantiated, e.g.:
    /// * For an `(entity, timeline, component)` index, that would be the last timestamp at which this
    ///   [`Chunk`] contains data for this particular component on this particular timeline (see
    ///   [`Chunk::time_range_per_component`]).
    /// * For an `(entity, timeline)` index, that would be the last timestamp at which this [`Chunk`]
    ///   contains data for any component on this particular timeline (see [`re_chunk::TimeColumn::time_range`]).
    pub(crate) per_end_time: BTreeMap<TimeInt, ChunkIdSet>,
}

pub type ChunkIdSetPerTimePerComponent = BTreeMap<ComponentName, ChunkIdSetPerTime>;

pub type ChunkIdSetPerTimePerComponentPerTimeline =
    BTreeMap<Timeline, ChunkIdSetPerTimePerComponent>;

pub type ChunkIdSetPerTimePerComponentPerTimelinePerEntity =
    BTreeMap<EntityPath, ChunkIdSetPerTimePerComponentPerTimeline>;

pub type ChunkIdPerComponent = BTreeMap<ComponentName, ChunkId>;

pub type ChunkIdPerComponentPerEntity = BTreeMap<EntityPath, ChunkIdPerComponent>;

pub type ChunkIdSetPerTimePerTimeline = BTreeMap<Timeline, ChunkIdSetPerTime>;

pub type ChunkIdSetPerTimePerTimelinePerEntity = BTreeMap<EntityPath, ChunkIdSetPerTimePerTimeline>;

// ---

/// Incremented on each edit.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChunkStoreGeneration {
    insert_id: u64,
    gc_id: u64,
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

    /// Keeps track of the _latest_ datatype information for all component types that have been written
    /// to the store so far.
    ///
    /// See also [`Self::lookup_datatype`].
    //
    // TODO(cmc): this would become fairly problematic in a world where each chunk can use a
    // different datatype for a given component.
    pub(crate) type_registry: IntMap<ComponentName, ArrowDataType>,

    pub(crate) chunks_per_chunk_id: BTreeMap<ChunkId, Arc<Chunk>>,

    /// All [`ChunkId`]s currently in the store, indexed by the smallest [`RowId`] in each of them.
    ///
    /// This is effectively all chunks in global data order. Used for garbage collection.
    ///
    /// This is a map of vecs instead of individual [`ChunkId`] in order to better support
    /// duplicated [`RowId`]s.
    pub(crate) chunk_ids_per_min_row_id: BTreeMap<RowId, Vec<ChunkId>>,

    /// All temporal [`ChunkId`]s for all entities on all timelines, further indexed by [`ComponentName`].
    ///
    /// See also:
    /// * [`Self::temporal_chunk_ids_per_entity`].
    /// * [`Self::static_chunk_ids_per_entity`].
    pub(crate) temporal_chunk_ids_per_entity_per_component:
        ChunkIdSetPerTimePerComponentPerTimelinePerEntity,

    /// All temporal [`ChunkId`]s for all entities on all timelines, without the [`ComponentName`] index.
    ///
    /// See also:
    /// * [`Self::temporal_chunk_ids_per_entity_per_component`].
    /// * [`Self::static_chunk_ids_per_entity`].
    pub(crate) temporal_chunk_ids_per_entity: ChunkIdSetPerTimePerTimelinePerEntity,

    /// Accumulated size statitistics for all temporal [`Chunk`]s currently present in the store.
    ///
    /// This is too costly to be computed from scratch every frame, and is required by e.g. the GC.
    pub(crate) temporal_chunks_stats: ChunkStoreChunkStats,

    /// Static data. Never garbage collected.
    ///
    /// Static data unconditionally shadows temporal data at query time.
    ///
    /// Existing temporal will not be removed. Events won't be fired.
    pub(crate) static_chunk_ids_per_entity: ChunkIdPerComponentPerEntity,

    /// Accumulated size statitistics for all static [`Chunk`]s currently present in the store.
    ///
    /// This is too costly to be computed from scratch every frame, and is required by e.g. the GC.
    pub(crate) static_chunks_stats: ChunkStoreChunkStats,

    // pub(crate) static_tables: BTreeMap<EntityPathHash, StaticTable>,
    /// Monotonically increasing ID for insertions.
    pub(crate) insert_id: u64,

    /// Monotonically increasing ID for queries.
    pub(crate) query_id: AtomicU64,

    /// Monotonically increasing ID for GCs.
    pub(crate) gc_id: u64,

    /// Monotonically increasing ID for store events.
    pub(crate) event_id: AtomicU64,
}

impl Clone for ChunkStore {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            type_registry: self.type_registry.clone(),
            chunks_per_chunk_id: self.chunks_per_chunk_id.clone(),
            chunk_ids_per_min_row_id: self.chunk_ids_per_min_row_id.clone(),
            temporal_chunk_ids_per_entity_per_component: self
                .temporal_chunk_ids_per_entity_per_component
                .clone(),
            temporal_chunk_ids_per_entity: self.temporal_chunk_ids_per_entity.clone(),
            temporal_chunks_stats: self.temporal_chunks_stats,
            static_chunk_ids_per_entity: self.static_chunk_ids_per_entity.clone(),
            static_chunks_stats: self.static_chunks_stats,
            insert_id: Default::default(),
            query_id: Default::default(),
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
            type_registry: _,
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id: chunk_id_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component: _,
            temporal_chunk_ids_per_entity: _,
            temporal_chunks_stats,
            static_chunk_ids_per_entity: _,
            static_chunks_stats,
            insert_id: _,
            query_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        f.write_str("ChunkStore {\n")?;

        f.write_str(&indent::indent_all_by(4, format!("id: {id}\n")))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        f.write_str(&indent::indent_all_by(4, "stats: {\n"))?;
        f.write_str(&indent::indent_all_by(
            8,
            format!("{}", *static_chunks_stats + *temporal_chunks_stats),
        ))?;
        f.write_str(&indent::indent_all_by(4, "}\n"))?;

        f.write_str(&indent::indent_all_by(4, "chunks: [\n"))?;
        for chunk_id in chunk_id_per_min_row_id.values().flatten() {
            if let Some(chunk) = chunks_per_chunk_id.get(chunk_id) {
                f.write_str(&indent::indent_all_by(8, format!("{chunk}\n")))?;
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
    #[inline]
    pub fn new(id: StoreId, config: ChunkStoreConfig) -> Self {
        Self {
            id,
            config,
            type_registry: Default::default(),
            chunk_ids_per_min_row_id: Default::default(),
            chunks_per_chunk_id: Default::default(),
            temporal_chunk_ids_per_entity_per_component: Default::default(),
            temporal_chunk_ids_per_entity: Default::default(),
            temporal_chunks_stats: Default::default(),
            static_chunk_ids_per_entity: Default::default(),
            static_chunks_stats: Default::default(),
            insert_id: 0,
            query_id: AtomicU64::new(0),
            gc_id: 0,
            event_id: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn id(&self) -> &StoreId {
        &self.id
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

    /// Lookup the _latest_ arrow [`ArrowDataType`] used by a specific [`re_types_core::Component`].
    #[inline]
    pub fn lookup_datatype(&self, component_name: &ComponentName) -> Option<&ArrowDataType> {
        self.type_registry.get(component_name)
    }
}
