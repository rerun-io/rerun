use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use re_chunk_store::{ChunkDeletionReason, ChunkStoreDiff, ChunkStoreEvent, RowId};
use re_log_types::hash::Hash64;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimelineName};
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::TextLog;
use re_view::ChunksWithComponent;
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::{Cache, ViewId};

use crate::text_log_level_cache::collect_levels;
use crate::text_log_query::text_levels;
use crate::text_log_source::TextLogSourceKey;
use crate::text_log_span_index::TextLogSpanIndex;
use crate::visualizer_system::TextLogSource;

/// Identifies a text log query for the purposes of caching index and text log level information.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub struct TextLogQueryKey(Hash64);

impl TextLogQueryKey {
    /// Assumes text and level are never supplied by blueprint overrides or view defaults.
    /// Both fields are marked `no_ui_edit`; their blueprint value revisions are not hashed.
    pub fn new(
        timeline: TimelineName,
        filter: Option<&BTreeSet<ArrowString>>,
        sources: &[TextLogSource<'_>],
    ) -> Self {
        use std::hash::{BuildHasher as _, Hash as _, Hasher as _};

        let mut hasher = re_log_types::hash::HASH_RANDOM_STATE.build_hasher();
        std::hash::Hash::hash(&timeline, &mut hasher);
        filter.hash(&mut hasher);

        for source in sources {
            let instruction = source.instruction;
            source.data_result.entity_path.hash().hash(&mut hasher);
            instruction.id.hash(&mut hasher);

            for component in [
                TextLog::descriptor_text().component,
                TextLog::descriptor_level().component,
            ] {
                instruction
                    .component_mappings
                    .get(&component)
                    .hash(&mut hasher);
            }
        }

        Self(Hash64::from_u64(hasher.finish()))
    }

    #[cfg(test)]
    pub fn for_test(value: impl std::hash::Hash) -> Self {
        Self(Hash64::hash(value))
    }
}

/// Cached ordering of text instances passing the level filter.
///
/// Covers one resolved span of the progressive navigation layout.
/// Row identities do not depend on chunk boundaries or display-only components.
#[derive(Default, re_byte_size::SizeBytes)]
pub struct TextLogIndex {
    /// Rows passing the level filter, sorted by time on the query timeline.
    pub rows: Vec<IndexedRow>,
}

/// A text instance identified independently of its current chunk or Arrow buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
pub struct IndexedRow {
    pub time: TimeInt,

    /// Zero-based position of the originating visualizer instruction in this index's query inputs.
    /// Distinguishes instructions that read the same recording row with different mappings;
    /// population must use the same input ordering as index construction.
    pub source: usize,
    pub row_id: RowId,

    /// Zero-based index of the text within the component batch identified by `row_id`.
    /// A recording row may contain multiple texts, each producing a separate table row.
    /// Filtering preserves this original index rather than renumbering surviving instances.
    pub instance: usize,
}

impl TextLogIndex {
    pub fn append(
        &mut self,
        source: usize,
        timeline: TimelineName,
        filter: Option<&BTreeSet<ArrowString>>,
        texts: &ChunksWithComponent<'_>,
        levels: &ChunksWithComponent<'_>,
        discovered: &mut BTreeSet<String>,
    ) {
        re_tracing::profile_function!();

        for row in text_levels(timeline, texts, levels) {
            collect_levels(discovered, row.num_instances, row.levels.as_deref());
            for instance in 0..row.num_instances {
                let level = row
                    .levels
                    .as_ref()
                    .and_then(|levels| levels.get(instance.min(levels.len().saturating_sub(1))));
                if let Some(level) = level
                    && filter.is_some_and(|filter| !filter.contains(level))
                {
                    continue;
                }
                self.rows.push(IndexedRow {
                    time: row.time,
                    source,
                    row_id: row.row_id,
                    instance,
                });
            }
        }
    }
}

/// Snapshot returned by [`TextLogIndexCache`] to plan navigation and query unresolved spans
/// outside the cache lock. Pass its `layout` to [`TextLogIndexCache::refine`] when publishing
/// queried rows so results are rejected if the source/layout identity changed in the meantime.
#[derive(Clone, re_byte_size::SizeBytes)]
pub struct TextLogCachedIndex {
    /// Estimated counts and bounded retained exact identities.
    pub index: Arc<TextLogSpanIndex>,

    /// Changes when previous span-relative anchors may refer to different rows.
    pub layout: RowId,
}

#[derive(re_byte_size::SizeBytes)]
struct CachedIndex {
    navigation: TextLogCachedIndex,

    sources: Vec<TextLogSourceKey>,

    /// Per-view (last protected frame, viewport range including overscan), preventing eviction
    /// of exact row identities needed by other views across this frame and the next.
    protected: HashMap<ViewId, (u64, AbsoluteTimeRange)>,

    last_used_frame: u64,
}

/// Store-scoped navigation metadata, independent of viewport payload residency.
#[derive(Default)]
pub struct TextLogIndexCache {
    entries: HashMap<TextLogQueryKey, CachedIndex>,
    frame: u64,
}

impl TextLogIndexCache {
    /// Retrieves navigation, seeding coverage for new query configurations.
    pub fn entry(
        &mut self,
        recording: &EntityDb,
        key: TextLogQueryKey,
        sources: &[TextLogSourceKey],
    ) -> TextLogCachedIndex {
        re_tracing::profile_function!();

        // Coverage seeds estimated spans without loading the text payloads.
        let entry = self.entries.entry(key).or_insert_with(|| {
            let manifest = recording.rrd_manifest_index().manifest();
            let engine = recording.storage_engine();
            let ranges = sources.iter().flat_map(|source| {
                source.text_ranges(manifest, engine.store(), AbsoluteTimeRange::EVERYTHING)
            });
            CachedIndex {
                navigation: TextLogCachedIndex {
                    index: Arc::new(TextLogSpanIndex::new(ranges)),
                    layout: RowId::new(),
                },
                sources: sources.to_vec(),
                protected: HashMap::new(),
                last_used_frame: self.frame,
            }
        });
        entry.last_used_frame = self.frame;

        entry.navigation.clone()
    }

    /// Protects a view's overscan across this frame and the next frame's execution order.
    /// Temporal ranges remain valid when concurrent refinement changes row offsets.
    pub fn protect(&mut self, key: TextLogQueryKey, view: ViewId, range: AbsoluteTimeRange) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.protected.insert(view, (self.frame, range));
        }
    }

    /// Publishes one completed query into the latest snapshot, preserving other views' work.
    /// Rejects queries whose source/layout identity changed while they executed outside the lock.
    pub fn refine(
        &mut self,
        key: TextLogQueryKey,
        layout: RowId,
        span_id: usize,
        index: Arc<TextLogIndex>,
    ) -> Option<TextLogCachedIndex> {
        let entry = self.entries.get_mut(&key)?;
        if entry.navigation.layout != layout {
            return None;
        }

        let protected = entry
            .protected
            .values()
            .map(|(_, range)| *range)
            .collect::<Vec<_>>();
        Arc::make_mut(&mut entry.navigation.index).resolve_protected(span_id, index, &protected);
        entry.last_used_frame = self.frame;

        Some(entry.navigation.clone())
    }
}

impl Cache for TextLogIndexCache {
    fn name(&self) -> &'static str {
        "TextLogIndexCache"
    }

    fn begin_frame(&mut self) {
        self.entries.retain(|_, entry| {
            entry.protected.retain(|_, (frame, _)| *frame == self.frame);
            entry.last_used_frame == self.frame
        });
        self.frame += 1;
    }

    fn purge_memory(&mut self) {
        self.entries.clear();
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], recording: &EntityDb) {
        re_tracing::profile_function!();

        let manifest = recording.rrd_manifest_index().manifest();
        if manifest.is_some() && !events.iter().any(|event| event.diff.is_virtual_addition()) {
            return;
        }

        let engine = recording.storage_engine();
        let store = engine.store();

        // Chunks whose rows were added or removed; compaction only rewrites existing rows.
        let changed: Vec<_> = if manifest.is_some() {
            Vec::new()
        } else {
            events
                .iter()
                .filter_map(|event| match &event.diff {
                    ChunkStoreDiff::Addition(add) => Some(&**add.delta_chunk()),
                    ChunkStoreDiff::Deletion(del) => {
                        (del.reason != ChunkDeletionReason::Compaction).then_some(&*del.chunk)
                    }
                    ChunkStoreDiff::VirtualAddition(_) | ChunkStoreDiff::SchemaAddition(_) => None,
                })
                .collect()
        };

        #[expect(
            clippy::iter_over_hash_type,
            reason = "entries are updated independently"
        )]
        for entry in self.entries.values_mut() {
            let dirty: Vec<_> = if manifest.is_some() {
                // Any manifest change just invalidetes everything - it's rare and just makes things easier.
                vec![AbsoluteTimeRange::EVERYTHING]
            } else {
                entry
                    .sources
                    .iter()
                    .flat_map(|source| source.dirty_ranges(&changed, store))
                    .collect()
            };
            if !dirty.is_empty() {
                let ranges = entry.sources.iter().flat_map(|source| {
                    source.text_ranges(manifest, store, AbsoluteTimeRange::EVERYTHING)
                });
                Arc::make_mut(&mut entry.navigation.index).reconcile(ranges, &dirty);
                // Dirty rows can invalidate anchors even when span boundaries stay unchanged.
                entry.navigation.layout = RowId::new();
            }
        }
    }
}

impl re_byte_size::MemUsageTreeCapture for TextLogIndexCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        use re_byte_size::SizeBytes as _;
        re_byte_size::MemUsageTree::Bytes(self.entries.total_size_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use re_chunk_store::Chunk;
    use re_log_types::Timeline;
    use re_sdk_types::archetypes::TextLog;

    use super::*;
    use crate::text_log_span_index::TARGET_SPAN_ROWS;

    // Level-only writes must not starve another source's refinement.
    #[test]
    fn level_updates_preserve_other_source_progress() {
        // Spans of this many rows never merge, so each chunk keeps its own span.
        let span_rows = TARGET_SPAN_ROWS;
        let mut db = database();
        for time in [0, 2] {
            db.add_chunk(&Arc::new(text_rows("logs", time, span_rows)))
                .unwrap();
        }
        let other = [1000, 2000].map(|time| text_rows("other", time, span_rows));
        for chunk in &other {
            db.add_chunk(&Arc::new(chunk.clone())).unwrap();
        }
        let sources = [source("logs"), source("other")];
        let mut cache = TextLogIndexCache::default();
        let initial = cache.entry(&db, key(), &sources);
        let first = initial
            .index
            .row_for_time(TimeInt::new_temporal(1000), false);
        let rows = first..first + other.len() * span_rows;
        cache.protect(
            key(),
            ViewId::random(),
            initial.index.protected_range(rows.clone()),
        );

        for (completed, chunk) in other.iter().enumerate() {
            add(&mut db, &mut cache, level_chunk("logs", -1, "WARN"));
            let cached = cache.entry(&db, key(), &sources);

            assert_eq!(
                cached
                    .index
                    .resolved_rows(rows.clone())
                    .map(|(_, rows)| rows.len())
                    .sum::<usize>(),
                completed * span_rows,
                "unrelated level updates must retain earlier viewport refinements"
            );
            let (span, _) = cached.index.next_query(rows.start, rows.end).unwrap();
            let exact = Arc::new(index(1, std::slice::from_ref(chunk)));
            cache.refine(key(), cached.layout, span, exact).unwrap();
        }

        assert!(
            cache
                .entry(&db, key(), &sources)
                .index
                .next_query(rows.start, rows.end)
                .is_none()
        );
    }

    #[test]
    fn overlapping_text_writes_preserve_earlier_same_source_refinement() {
        let mut db = database();
        let first = text_chunk("logs", 10);
        db.add_chunk(&Arc::new(first.clone())).unwrap();
        let sources = [source("logs")];
        let mut cache = TextLogIndexCache::default();
        let initial = cache.entry(&db, key(), &sources);
        let exact = Arc::new(index(0, std::slice::from_ref(&first)));
        cache
            .refine(key(), initial.layout, 0, exact.clone())
            .unwrap();

        add(&mut db, &mut cache, text_chunk("logs", 1000));
        let updated = cache.entry(&db, key(), &sources);

        assert_eq!(
            updated.index.resolved_rows(0..1).next().unwrap().1,
            exact.rows,
            "overlapping late writes must retain the earlier row identity"
        );
        assert!(updated.index.next_query(0, 1).is_none());
    }

    // Compaction rewrites physical chunks, but only newly appended rows need querying.
    #[test]
    fn compaction_preserves_resolved_rows_outside_the_append() {
        let mut db = EntityDb::new(re_log_types::StoreId::random(
            re_log_types::StoreKind::Recording,
            "text-log-cache",
        ));
        let first = text_chunk("logs", 10);
        db.add_chunk(&Arc::new(first.clone())).unwrap();
        let mut cache = TextLogIndexCache::default();
        let sources = [source("logs")];
        let initial = cache.entry(&db, key(), &sources);
        let exact = Arc::new(index(0, std::slice::from_ref(&first)));
        cache
            .refine(key(), initial.layout, 0, exact.clone())
            .unwrap();

        let events = add(&mut db, &mut cache, text_chunk("logs", 20));
        assert!(
            events
                .iter()
                .filter_map(|event| event.to_addition())
                .any(|add| matches!(
                    add.direct_lineage,
                    re_chunk_store::ChunkDirectLineageReport::CompactedFrom(_)
                )),
            "fixture must compact the existing chunk"
        );

        let updated = cache.entry(&db, key(), &sources);
        assert_eq!(
            updated.index.resolved_rows(0..1).next().unwrap().1,
            exact.rows
        );
        let row = updated.index.row_for_time(TimeInt::new_temporal(20), false);
        assert!(updated.index.next_query(row, row + 1).is_some());
    }

    // Manifest metadata supplies coverage even when no payloads are resident.
    #[test]
    fn manifest_extension_invalidates_exact_rows_and_rejects_stale_refinement() {
        let mut db = database();
        let chunks = [10, 20].map(|time| text_chunk("logs", time));
        db.add_rrd_manifest_message(manifest(&db, &chunks[..1]));
        let sources = [source("logs")];
        let mut cache = TextLogIndexCache::default();
        let initial = cache.entry(&db, key(), &sources);
        assert_eq!(initial.index.len(), 1);
        let exact = Arc::new(index(0, &chunks[..1]));
        cache
            .refine(key(), initial.layout, 0, exact.clone())
            .unwrap();

        let events = db.add_rrd_manifest_message(manifest(&db, &chunks[1..]));
        cache.on_store_events(&events.iter().collect::<Vec<_>>(), &db);

        assert!(cache.refine(key(), initial.layout, 0, exact).is_none());
        let updated = cache.entry(&db, key(), &sources);
        assert_ne!(initial.layout, updated.layout);
        assert_eq!(updated.index.len(), 2);
        assert!(updated.index.resolved_rows(0..2).next().is_none());
    }

    #[test]
    fn resident_and_manifest_text_ranges_select_the_same_coverage() {
        let mut db = database();
        let chunks = [10, 20, 30].map(|time| text_chunk("logs", time));
        let manifest = manifest(&db, &chunks);
        for chunk in chunks {
            db.add_chunk(&Arc::new(chunk)).unwrap();
        }
        let engine = db.storage_engine();
        let source = source("logs");

        for manifest in [None, Some(manifest.as_ref())] {
            let ranges =
                source.text_ranges(manifest, engine.store(), AbsoluteTimeRange::new(15, 25));
            assert_eq!(
                ranges,
                vec![(AbsoluteTimeRange::new(20, 20), 1)],
                "only intersecting text coverage should be returned, manifest={}",
                manifest.is_some()
            );
        }
    }

    fn source(entity: &str) -> TextLogSourceKey {
        TextLogSourceKey {
            entity: entity.into(),
            timeline: TimelineName::log_tick(),
            text: TextLog::descriptor_text().component,
            level: TextLog::descriptor_level().component,
        }
    }

    fn key() -> TextLogQueryKey {
        TextLogQueryKey::for_test(())
    }

    fn database() -> EntityDb {
        EntityDb::with_store_config(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "text-log-cache"),
            true,
            re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
        )
    }

    fn manifest(db: &EntityDb, chunks: &[Chunk]) -> Arc<re_chunk_index::RrdManifest> {
        re_chunk_index::RrdManifest::build_in_memory_from_chunks(
            db.store_id().clone(),
            chunks.iter(),
        )
        .unwrap()
    }

    /// Adds `chunk` and forwards the resulting store events to `cache`.
    fn add(db: &mut EntityDb, cache: &mut TextLogIndexCache, chunk: Chunk) -> Vec<ChunkStoreEvent> {
        let events = db.add_chunk(&Arc::new(chunk)).unwrap();
        cache.on_store_events(&events.iter().collect::<Vec<_>>(), db);
        events
    }

    fn text_chunk(entity: &str, time: i64) -> Chunk {
        text_rows(entity, time, 1)
    }

    fn text_rows(entity: &str, time: i64, rows: usize) -> Chunk {
        (0..rows)
            .fold(Chunk::builder(entity), |builder, _| {
                builder
                    .with_archetype_auto_row([(Timeline::log_tick(), time)], &TextLog::new("body"))
            })
            .build()
            .unwrap()
    }

    fn level_chunk(entity: &str, time: i64, level: &str) -> Chunk {
        Chunk::builder(entity)
            .with_archetype_auto_row(
                [(Timeline::log_tick(), time)],
                &TextLog::update_fields().with_level(level),
            )
            .build()
            .unwrap()
    }

    fn index(source: usize, chunks: &[Chunk]) -> TextLogIndex {
        let texts = re_view::ChunksWithComponent {
            chunks: Cow::Borrowed(chunks),
            component: TextLog::descriptor_text().component,
        };
        let levels = re_view::ChunksWithComponent {
            chunks: Cow::Borrowed(chunks),
            component: TextLog::descriptor_level().component,
        };
        let mut index = TextLogIndex::default();
        index.append(
            source,
            TimelineName::log_tick(),
            None,
            &texts,
            &levels,
            &mut BTreeSet::new(),
        );
        index.rows.sort_by_key(|row| row.time);
        index
    }
}
