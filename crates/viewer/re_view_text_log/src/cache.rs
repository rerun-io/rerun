use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use re_byte_size::{MemUsageTree, MemUsageTreeCapture, SizeBytes};
use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimelineName};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_viewer_context::Cache;

/// Lightweight metadata for one visible text-log instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLogRowMeta {
    pub row_idx: usize,
    pub instance_idx: usize,
    pub row_id: RowId,
    pub timepoint: TimePoint,
    pub level: Option<TextLogLevel>,
    pub color: Option<Color>,
    pub line_count: u32,
}

impl SizeBytes for TextLogRowMeta {
    /// Reports the heap usage of the cloned metadata we keep per text-log instance.
    fn heap_size_bytes(&self) -> u64 {
        self.timepoint.heap_size_bytes()
            + self.level.heap_size_bytes()
            + self.color.heap_size_bytes()
    }

    /// Reports that row metadata is not plain old data because it owns heap allocations.
    fn is_pod() -> bool {
        false
    }
}

/// Sort key plus chunk-local lookup information for one projected text-log row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLogRowHandle {
    pub chunk_id: ChunkId,
    pub row_meta_idx: usize,
    pub sort_time: TimeInt,
    pub row_id: RowId,
    pub instance_idx: usize,
}

impl PartialOrd for TextLogRowHandle {
    /// Compares row handles by the same stable key used by the legacy eager table.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextLogRowHandle {
    /// Compares row handles by `(sort_time, chunk_id, row_id, instance_idx)`.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.sort_time, self.chunk_id, self.row_id, self.instance_idx).cmp(&(
            other.sort_time,
            other.chunk_id,
            other.row_id,
            other.instance_idx,
        ))
    }
}

/// Indexed representation of one chunk that contains text-log data.
#[derive(Debug)]
pub struct IndexedTextLogChunk {
    pub chunk: Arc<Chunk>,
    pub row_metas: Vec<TextLogRowMeta>,
}

impl IndexedTextLogChunk {
    /// Returns the entity path shared by all rows in this indexed chunk.
    pub fn entity_path(&self) -> &EntityPath {
        self.chunk.entity_path()
    }

    /// Returns one row's cached metadata by index.
    pub fn row_meta(&self, row_meta_idx: usize) -> Option<&TextLogRowMeta> {
        self.row_metas.get(row_meta_idx)
    }

    /// Appends projected row handles for a specific timeline.
    pub fn append_row_handles_for_timeline(
        &self,
        timeline: TimelineName,
        handles: &mut Vec<TextLogRowHandle>,
    ) {
        let chunk_id = self.chunk.id();
        handles.extend(self.row_metas.iter().enumerate().map(|(row_meta_idx, row_meta)| {
            TextLogRowHandle {
                chunk_id,
                row_meta_idx,
                sort_time: row_sort_time(&row_meta.timepoint, timeline),
                row_id: row_meta.row_id,
                instance_idx: row_meta.instance_idx,
            }
        }));
    }

    /// Resolves the text body for one projected row on demand.
    pub fn resolve_body(&self, row_meta_idx: usize) -> Option<Text> {
        let row_meta = self.row_meta(row_meta_idx)?;
        let bodies = self
            .chunk
            .component_batch::<Text>(TextLog::descriptor_text().component, row_meta.row_idx)?
            .ok()?;

        bodies.get(row_meta.instance_idx).cloned()
    }
}

impl SizeBytes for IndexedTextLogChunk {
    /// Reports the heap owned by the cached row metadata for one indexed chunk.
    fn heap_size_bytes(&self) -> u64 {
        self.row_metas.heap_size_bytes()
    }

    /// Reports that indexed chunks are not plain old data because they own metadata vectors.
    fn is_pod() -> bool {
        false
    }
}

/// Cache of indexed text-log chunks for the active store.
#[derive(Default)]
pub struct TextLogCache {
    chunks: BTreeMap<ChunkId, Arc<IndexedTextLogChunk>>,
    addition_log: Vec<(u64, ChunkId)>,
    revision: u64,
    non_additive_revision: u64,
    initialized: bool,
    needs_rebuild: bool,
}

impl TextLogCache {
    /// Ensures that the cache has been seeded from the current store contents.
    pub fn ensure_initialized(&mut self, entity_db: &EntityDb) {
        if self.initialized && !self.needs_rebuild {
            return;
        }

        self.rebuild_from_store(entity_db);
    }

    /// Returns the current cache revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Reports whether every change since `revision` was append-only.
    pub fn is_additive_since(&self, revision: u64) -> bool {
        revision >= self.non_additive_revision && revision <= self.revision
    }

    /// Collects indexed chunks for the currently included entities.
    pub fn collect_chunks_for_entities(
        &self,
        included_entities: &std::collections::BTreeSet<EntityPath>,
    ) -> Vec<Arc<IndexedTextLogChunk>> {
        self.chunks
            .values()
            .filter(|chunk| included_entities.contains(chunk.entity_path()))
            .cloned()
            .collect()
    }

    /// Collects append-only chunk additions that happened after `revision`.
    pub fn collect_added_chunks_since(
        &self,
        included_entities: &std::collections::BTreeSet<EntityPath>,
        revision: u64,
    ) -> Vec<Arc<IndexedTextLogChunk>> {
        self.addition_log
            .iter()
            .filter(|(entry_revision, _)| *entry_revision > revision)
            .filter_map(|(_, chunk_id)| self.chunks.get(chunk_id))
            .filter(|chunk| included_entities.contains(chunk.entity_path()))
            .cloned()
            .collect()
    }

    /// Rebuilds the cache from the currently loaded physical chunks.
    fn rebuild_from_store(&mut self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let store = entity_db.storage_engine();
        let physical_chunks = store
            .store()
            .iter_physical_chunks()
            .cloned()
            .collect::<Vec<_>>();

        self.rebuild_from_chunks(physical_chunks);
    }

    /// Rebuilds the cache from a supplied chunk list.
    fn rebuild_from_chunks(&mut self, chunks: impl IntoIterator<Item = Arc<Chunk>>) {
        let chunks = chunks
            .into_iter()
            .filter_map(Self::index_chunk)
            .map(|chunk| (chunk.chunk.id(), chunk))
            .collect::<BTreeMap<_, _>>();

        self.chunks = chunks;
        self.addition_log.clear();
        self.initialized = true;
        self.needs_rebuild = false;

        if self.revision == 0 {
            self.revision = 1;
            self.non_additive_revision = self.revision;
        }
    }

    /// Adds a newly appended chunk without rebuilding the existing index.
    fn append_delta_chunk(&mut self, chunk: Arc<Chunk>) {
        let Some(indexed_chunk) = Self::index_chunk(chunk) else {
            return;
        };

        self.revision += 1;
        self.addition_log
            .push((self.revision, indexed_chunk.chunk.id()));
        self.chunks.insert(indexed_chunk.chunk.id(), indexed_chunk);
    }

    /// Marks the cache as needing a full rebuild before the next access.
    fn mark_non_additive(&mut self) {
        self.revision += 1;
        self.non_additive_revision = self.revision;
        self.needs_rebuild = true;
        self.addition_log.clear();
    }

    /// Builds an indexed chunk by expanding only the lightweight metadata we need for drawing.
    fn index_chunk(chunk: Arc<Chunk>) -> Option<Arc<IndexedTextLogChunk>> {
        re_tracing::profile_function!();

        let text_component = TextLog::descriptor_text().component;
        let level_component = TextLog::descriptor_level().component;
        let color_component = TextLog::descriptor_color().component;

        if chunk.num_events_for_component(text_component).unwrap_or(0) == 0 {
            return None;
        }

        let row_ids = chunk.row_ids_slice();
        let mut row_metas = Vec::new();
        let mut latest_levels: Option<Vec<TextLogLevel>> = None;
        let mut latest_colors: Option<Vec<Color>> = None;

        for (row_idx, timepoint) in chunk.iter_timepoints().enumerate() {
            if let Some(Ok(levels)) = chunk.component_batch::<TextLogLevel>(level_component, row_idx)
                && !levels.is_empty()
            {
                latest_levels = Some(levels);
            }

            if let Some(Ok(colors)) = chunk.component_batch::<Color>(color_component, row_idx)
                && !colors.is_empty()
            {
                latest_colors = Some(colors);
            }

            let Some(Ok(bodies)) = chunk.component_batch::<Text>(text_component, row_idx) else {
                continue;
            };

            if bodies.is_empty() {
                continue;
            }

            let row_id = row_ids[row_idx];
            let level_slice = latest_levels.as_deref();
            let color_slice = latest_colors.as_deref();

            for (instance_idx, body) in bodies.into_iter().enumerate() {
                row_metas.push(TextLogRowMeta {
                    row_idx,
                    instance_idx,
                    row_id,
                    timepoint: timepoint.clone(),
                    level: clamped_value(level_slice, instance_idx),
                    color: clamped_value(color_slice, instance_idx),
                    line_count: explicit_line_count(body.as_str()),
                });
            }
        }

        (!row_metas.is_empty()).then_some(Arc::new(IndexedTextLogChunk { chunk, row_metas }))
    }
}

impl Cache for TextLogCache {
    /// Returns the cache name used by viewer diagnostics.
    fn name(&self) -> &'static str {
        "TextLogCache"
    }

    /// Keeps the current index alive because it is the steady-state performance win.
    fn purge_memory(&mut self) {}

    /// Applies store events incrementally while falling back to rebuilds on non-additive changes.
    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        if !self.initialized || self.needs_rebuild {
            return;
        }

        for event in events {
            if event.diff.is_deletion() {
                self.mark_non_additive();
                return;
            }

            let Some(delta_chunk) = event.diff.delta_chunk() else {
                continue;
            };

            if event.diff.is_addition() {
                self.append_delta_chunk(Arc::clone(delta_chunk));
            }
        }
    }
}

impl MemUsageTreeCapture for TextLogCache {
    /// Reports the memory owned by the lightweight text-log index structures.
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        MemUsageTree::Bytes(
            self.chunks
                .values()
                .map(|chunk| chunk.heap_size_bytes())
                .sum::<u64>()
                + self.addition_log.heap_size_bytes()
        )
    }
}

/// Returns the sort key used by the text-log table for one row.
pub fn row_sort_time(timepoint: &TimePoint, timeline: TimelineName) -> TimeInt {
    timepoint
        .get(&timeline)
        .map(TimeInt::from)
        .unwrap_or(TimeInt::STATIC)
}

/// Clamps a slice exactly like the old eager visualizer did for per-instance values.
fn clamped_value<T: Clone>(values: Option<&[T]>, instance_idx: usize) -> Option<T> {
    let values = values?;
    values
        .get(instance_idx)
        .cloned()
        .or_else(|| values.last().cloned())
}

/// Counts explicit lines exactly like the old row-height code path.
fn explicit_line_count(body: &str) -> u32 {
    (1 + body.bytes().filter(|&byte| byte == b'\n').count()) as u32
}

#[cfg(test)]
mod tests {
    use super::{TextLogCache, TextLogRowMeta, explicit_line_count};
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, Timeline};
    use re_sdk_types::archetypes::TextLog;
    use re_sdk_types::components::{Color, Text, TextLogLevel};

    /// Builds a small text-log chunk with sparse rows for indexing tests.
    fn sparse_text_log_chunk() -> Arc<Chunk> {
        let mut chunk = Chunk::builder("logs/test")
            .with_component_batches(
                RowId::new(),
                [(Timeline::log_time(), 1)],
                [(
                    TextLog::descriptor_text(),
                    &[Text::from("alpha"), Text::from("beta\nbeta")] as _,
                )],
            )
            .with_component_batches(
                RowId::new(),
                [(Timeline::log_time(), 2)],
                [(
                    TextLog::descriptor_level(),
                    &[TextLogLevel::from(TextLogLevel::WARN)] as _,
                )],
            )
            .with_component_batches(
                RowId::new(),
                [(Timeline::log_time(), 3)],
                [
                    (
                        TextLog::descriptor_text(),
                        &[Text::from("gamma"), Text::from("delta")] as _,
                    ),
                    (
                        TextLog::descriptor_color(),
                        &[Color::from(0xFF0000FF), Color::from(0x00FF00FF)] as _,
                    ),
                ],
            )
            .build()
            .expect("chunk should build");

        chunk.sort_if_unsorted();
        Arc::new(chunk)
    }

    /// Returns the indexed metadata for the sparse test chunk.
    fn sparse_row_metas() -> Vec<TextLogRowMeta> {
        TextLogCache::index_chunk(sparse_text_log_chunk())
            .expect("chunk should be indexed")
            .row_metas
            .clone()
    }

    /// Verifies that indexing preserves sparse carry-forward semantics and newline counting.
    #[test]
    fn index_chunk_matches_old_clamping_behavior() {
        let row_metas = sparse_row_metas();

        assert_eq!(row_metas.len(), 4);
        assert_eq!(row_metas[0].level, None);
        assert_eq!(row_metas[1].level, None);
        assert_eq!(
            row_metas[2].level.as_ref().map(TextLogLevel::as_str),
            Some(TextLogLevel::WARN)
        );
        assert_eq!(
            row_metas[3].level.as_ref().map(TextLogLevel::as_str),
            Some(TextLogLevel::WARN)
        );
        assert_eq!(row_metas[0].color, None);
        assert_eq!(row_metas[1].color, None);
        assert_eq!(row_metas[2].color, Some(Color::from(0xFF0000FF)));
        assert_eq!(row_metas[3].color, Some(Color::from(0x00FF00FF)));
        assert_eq!(row_metas[0].line_count, 1);
        assert_eq!(row_metas[1].line_count, 2);
    }

    /// Verifies that visible body resolution still reads the original chunk data lazily.
    #[test]
    fn indexed_chunk_resolves_body_by_row_and_instance() {
        let chunk = TextLogCache::index_chunk(sparse_text_log_chunk()).expect("chunk should exist");

        assert_eq!(
            chunk.resolve_body(1).as_ref().map(Text::as_str),
            Some("beta\nbeta")
        );
        assert_eq!(chunk.resolve_body(2).as_ref().map(Text::as_str), Some("gamma"));
        assert_eq!(chunk.resolve_body(3).as_ref().map(Text::as_str), Some("delta"));
    }

    /// Verifies that the cache tracks additive updates separately from rebuild-required changes.
    #[test]
    fn cache_revisions_distinguish_additive_and_non_additive_updates() {
        let mut cache = TextLogCache::default();
        cache.rebuild_from_chunks([sparse_text_log_chunk()]);

        let initial_revision = cache.revision();
        assert_eq!(initial_revision, 1);
        assert!(!cache.is_additive_since(0));
        assert!(cache.is_additive_since(initial_revision));

        cache.append_delta_chunk(
            Chunk::builder("logs/extra")
                .with_component_batches(
                    RowId::new(),
                    [(Timeline::log_time(), 4)],
                    [(
                        TextLog::descriptor_text(),
                        &[Text::from("epsilon")] as _,
                    )],
                )
                .build()
                .expect("chunk should build")
                .into(),
        );

        let additive_revision = cache.revision();
        assert!(cache.is_additive_since(initial_revision));
        assert_eq!(cache.collect_added_chunks_since(&["logs/extra".into()].into_iter().collect(), initial_revision).len(), 1);

        cache.mark_non_additive();

        assert!(!cache.is_additive_since(additive_revision));
    }

    /// Verifies the explicit newline counter used by cached row heights.
    #[test]
    fn explicit_line_count_counts_visual_lines() {
        assert_eq!(explicit_line_count(""), 1);
        assert_eq!(explicit_line_count("single"), 1);
        assert_eq!(explicit_line_count("two\nlines"), 2);
        assert_eq!(explicit_line_count("three\nvisible\nlines"), 3);
    }
}
