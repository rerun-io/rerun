use std::sync::{atomic::Ordering, Arc};

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ComponentName, EntityPath, Timeline};

use crate::ChunkStore;

// ---

#[derive(Default, Debug, Clone, Copy)]
pub struct ChunkStoreStats {
    pub static_chunks: ChunkStoreChunkStats,
    pub temporal_chunks: ChunkStoreChunkStats,
}

impl ChunkStoreStats {
    #[inline]
    pub fn total(&self) -> ChunkStoreChunkStats {
        let Self {
            static_chunks,
            temporal_chunks,
        } = *self;
        static_chunks + temporal_chunks
    }
}

impl std::ops::Add for ChunkStoreStats {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
        } = self;

        let static_chunks = static_chunks + rhs.static_chunks;
        let temporal_chunks = temporal_chunks + rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
        }
    }
}

impl std::ops::Sub for ChunkStoreStats {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
        } = self;

        let static_chunks = static_chunks - rhs.static_chunks;
        let temporal_chunks = temporal_chunks - rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
        }
    }
}

impl ChunkStore {
    #[inline]
    pub fn stats(&self) -> ChunkStoreStats {
        ChunkStoreStats {
            static_chunks: self.static_chunks_stats,
            temporal_chunks: self.temporal_chunks_stats,
        }
    }
}

// ---

/// Stats about a collection of chunks.
///
/// Each chunk contains data for only one entity.
///
/// Each chunk has data for either zero timelines (static chunk) or multiple timelines (temporal chunk).
/// A temporal chunk has dense timelines.
///
/// Each chunk can contain multiple components (columns).
#[derive(Default, Debug, Clone, Copy)]
pub struct ChunkStoreChunkStats {
    /// The number of chunks this is the stats for.
    pub num_chunks: u64,

    /// Includes everything: arrow payloads, timelines, rowids, and chunk overhead.
    ///
    /// This is an approximation of the actual storage cost of an entity,
    /// as the measurement includes the overhead of various data structures
    /// we use in the database.
    /// It is imprecise, because it does not account for every possible place
    /// someone may be storing something related to the entity, only most of
    /// what is accessible inside this chunk store.
    pub total_size_bytes: u64,

    /// Number of rows.
    ///
    /// This is usually the same as the number of log calls the user made.
    /// Each row can contain multiple events (see [`Self::num_events`]).
    pub num_rows: u64,

    /// How many _component batches_ ("cells").
    pub num_events: u64,
}

impl std::fmt::Display for ChunkStoreChunkStats {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            num_chunks,
            total_size_bytes,
            num_rows,
            num_events,
        } = *self;

        f.write_fmt(format_args!(
            "num_chunks: {}\n",
            re_format::format_uint(num_chunks)
        ))?;
        f.write_fmt(format_args!(
            "total_size_bytes: {}\n",
            re_format::format_bytes(total_size_bytes as _)
        ))?;
        f.write_fmt(format_args!(
            "num_rows: {}\n",
            re_format::format_uint(num_rows)
        ))?;
        f.write_fmt(format_args!(
            "num_events: {}\n",
            re_format::format_uint(num_events)
        ))?;

        Ok(())
    }
}

impl std::ops::Add for ChunkStoreChunkStats {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks + rhs.num_chunks,
            total_size_bytes: self.total_size_bytes + rhs.total_size_bytes,
            num_rows: self.num_rows + rhs.num_rows,
            num_events: self.num_events + rhs.num_events,
        }
    }
}

impl std::ops::AddAssign for ChunkStoreChunkStats {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub for ChunkStoreChunkStats {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks - rhs.num_chunks,
            total_size_bytes: self.total_size_bytes - rhs.total_size_bytes,
            num_rows: self.num_rows - rhs.num_rows,
            num_events: self.num_events - rhs.num_events,
        }
    }
}

impl std::ops::SubAssign for ChunkStoreChunkStats {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::iter::Sum for ChunkStoreChunkStats {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = Self::default();
        for item in iter {
            sum += item;
        }
        sum
    }
}

impl ChunkStoreChunkStats {
    #[inline]
    pub fn from_chunk(chunk: &Arc<Chunk>) -> Self {
        // NOTE: Do _NOT_ use `chunk.total_size_bytes` as it is sitting behind an Arc
        // and would count as amortized (i.e. 0 bytes).
        let size_bytes = <Chunk as SizeBytes>::total_size_bytes(&**chunk);

        Self {
            num_chunks: 1,
            total_size_bytes: size_bytes,
            num_rows: chunk.num_rows() as u64,
            num_events: chunk.num_events_cumulative(),
        }
    }
}

// ----------------------------------------------------------------------------

/// ## Entity stats
impl ChunkStore {
    /// Stats about all chunks with static data for an entity.
    pub fn entity_stats_static(&self, entity_path: &EntityPath) -> ChunkStoreChunkStats {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.static_chunk_ids_per_entity.get(entity_path).map_or(
            ChunkStoreChunkStats::default(),
            |static_chunks_per_component| {
                let chunk_ids: ahash::HashSet<re_chunk::ChunkId> =
                    static_chunks_per_component.values().copied().collect();

                chunk_ids
                    .into_iter()
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(&chunk_id))
                    .map(ChunkStoreChunkStats::from_chunk)
                    .sum()
            },
        )
    }

    /// Stats about all the chunks that has data for an entity on a specific timeline.
    ///
    /// Does NOT include static data.
    pub fn entity_stats_on_timeline(
        &self,
        entity_path: &EntityPath,
        timeline: &Timeline,
    ) -> ChunkStoreChunkStats {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .map_or(
                ChunkStoreChunkStats::default(),
                |chunk_id_sets| -> ChunkStoreChunkStats {
                    chunk_id_sets
                        .per_start_time
                        .values()
                        .flat_map(|chunk_ids| chunk_ids.iter())
                        .filter_map(|id| self.chunks_per_chunk_id.get(id))
                        .map(ChunkStoreChunkStats::from_chunk)
                        .sum()
                },
            )
    }
}

/// ## Component path stats
impl ChunkStore {
    /// Returns the number of static events logged for an entity for a specific component.
    ///
    /// This ignores temporal events.
    pub fn num_static_events_for_component(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(&component_name)
            })
            .and_then(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
            .and_then(|chunk| chunk.num_events_for_component(component_name))
            .unwrap_or(0)
    }

    /// Returns the number of temporal events logged for an entity for a specific component on a given timeline.
    ///
    /// This ignores static events.
    pub fn num_temporal_events_for_component_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .and_then(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.get(&component_name)
            })
            .map_or(0, |chunk_id_sets| {
                chunk_id_sets
                    .per_start_time
                    .values()
                    .flat_map(|chunk_ids| chunk_ids.iter())
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
                    .filter_map(|chunk| chunk.num_events_for_component(component_name))
                    .sum()
            })
    }

    /// Returns the number of temporal events logged for an entity for a specific component on all timelines.
    ///
    /// This ignores static events.
    pub fn num_temporal_events_for_component_on_all_timelines(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> u64 {
        self.all_timelines()
            .iter()
            .map(|timeline| {
                self.num_temporal_events_for_component_on_timeline(
                    timeline,
                    entity_path,
                    component_name,
                )
            })
            .sum()
    }
}
