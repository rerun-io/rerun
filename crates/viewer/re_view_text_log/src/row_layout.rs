use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use re_chunk_store::external::arrow::array::{
    Array as _, ListArray as ArrowListArray, StringArray as ArrowStringArray,
};
use re_chunk_store::{
    AbsoluteTimeRange, Chunk, ChunkId, ChunkStore, ChunkTrackingMode, RangeQuery, TimeInt,
};
use re_entity_db::EntityPath;
use re_log_types::TimelineName;
use re_sdk_types::ComponentIdentifier;

/// An explicit log level filter: only rows whose level is in the set are shown.
///
/// Rows without a level always pass.
#[derive(Clone, PartialEq, Eq)]
pub struct LevelFilter(pub BTreeSet<String>);

impl LevelFilter {
    pub fn matches(&self, level: Option<&str>) -> bool {
        level.is_none_or(|lvl| self.0.contains(lvl))
    }
}

/// Per-level row counts for a single chunk.
///
/// Chunks are immutable, so these can be cached indefinitely (keyed by [`ChunkId`]),
/// which makes level-filtered row counts as cheap as unfiltered ones after the first frame.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LevelCounts {
    /// Number of text log rows whose (first) level value is the given string.
    per_level: BTreeMap<String, u64>,

    /// Number of text log rows without a level value.
    unleveled: u64,
}

impl LevelCounts {
    fn matching(&self, filter: &LevelFilter) -> u64 {
        self.unleveled
            + filter
                .0
                .iter()
                .filter_map(|lvl| self.per_level.get(lvl))
                .sum::<u64>()
    }
}

impl re_byte_size::SizeBytes for LevelCounts {
    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(&self.per_level)
    }
}

/// Cached per-chunk level counts, keyed by chunk id.
pub type LevelCountCache = BTreeMap<ChunkId, LevelCounts>;

/// The contiguous span of table rows contributed by one chunk.
struct ChunkSpan {
    /// Time range covered by the chunk on the layout's timeline.
    ///
    /// Meaningless for static chunks ([`AbsoluteTimeRange`] cannot represent static times).
    time_range: AbsoluteTimeRange,

    /// Number of table rows this chunk contributes (only filter-matching rows count).
    num_rows: u64,

    is_static: bool,

    chunk: Arc<Chunk>,
}

/// Row layout of the text log table, derived from chunk-level metadata only.
///
/// This is what allows the view to know how many rows there are — and roughly where in time
/// each row lives — without touching any row data:
/// row counts come from the per-chunk validity bitmaps (or, with a level filter active,
/// from cached per-chunk level counts) and are exact,
/// and only locating a time window boundary *inside* a chunk requires peeking at that
/// (already loaded) chunk's time column.
///
/// Rows are ordered with static rows first, then by time.
/// Where chunks of different entities overlap in time, the layout is approximate at chunk
/// granularity, but stable; the visible window itself is always rendered in exact time order.
pub struct RowLayout {
    timeline: TimelineName,
    component: ComponentIdentifier,
    level_component: ComponentIdentifier,

    /// The active level filter; when set, all row counts only cover matching rows.
    filter: Option<LevelFilter>,

    /// Sorted by `time_range.min` (which puts static chunks first).
    chunks: Vec<ChunkSpan>,

    /// Prefix sums of `chunks[..i].num_rows`; length is `chunks.len() + 1`.
    prefix_rows: Vec<u64>,

    num_static_rows: u64,

    /// All log levels that occur anywhere in the considered chunks.
    levels: BTreeSet<String>,

    /// Chunks that the store knows of but that aren't loaded; row counts are incomplete.
    //
    // TODO(#7562): once this is fed from the `RrdManifestIndex`, unloaded chunks can
    // contribute their row counts too, making the layout complete in streaming scenarios.
    pub any_missing: bool,
}

impl RowLayout {
    /// Note that the level counting is done on the raw store data: blueprint overrides of the
    /// `level` component are not taken into account by `filter`.
    pub fn build<'a>(
        store: &ChunkStore,
        timeline: &TimelineName,
        component: ComponentIdentifier,
        level_component: ComponentIdentifier,
        entities: impl Iterator<Item = &'a EntityPath>,
        filter: Option<&LevelFilter>,
        level_counts: &mut LevelCountCache,
    ) -> Self {
        re_tracing::profile_function!();

        let query = RangeQuery::new(*timeline, AbsoluteTimeRange::EVERYTHING);

        let mut chunks = Vec::new();
        let mut levels = BTreeSet::new();
        let mut any_missing = false;
        let mut live_chunks = BTreeSet::new();

        for entity_path in entities {
            // `Ignore`: don't trigger chunk downloads just to compute the row layout.
            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Ignore,
                &query,
                entity_path,
                component,
            );
            any_missing |= !results.missing_virtual.is_empty();

            for chunk in results.chunks {
                let Some(total_rows) = chunk.num_events_for_component(component) else {
                    continue;
                };
                if total_rows == 0 {
                    continue;
                }

                live_chunks.insert(chunk.id());

                // One-time scan per chunk (chunks are immutable, so the cache never goes
                // stale); this is what makes level-filtered row counts affordable, and it
                // gives us the complete set of occurring levels for the filter UI.
                let counts = level_counts
                    .entry(chunk.id())
                    .or_insert_with(|| level_counts_for_chunk(&chunk, component, level_component));
                levels.extend(counts.per_level.keys().cloned());

                let num_rows = match filter {
                    Some(filter) => counts.matching(filter),
                    None => total_rows,
                };
                if num_rows == 0 {
                    continue;
                }

                let is_static = chunk.is_static();
                let time_range = if is_static {
                    AbsoluteTimeRange::EMPTY
                } else if let Some(time_column) = chunk.timelines().get(timeline) {
                    time_column.time_range()
                } else {
                    continue;
                };

                chunks.push(ChunkSpan {
                    time_range,
                    num_rows,
                    is_static,
                    chunk,
                });
            }
        }

        // Drop cache entries for chunks that no longer exist (compacted or garbage collected).
        level_counts.retain(|chunk_id, _| live_chunks.contains(chunk_id));

        // Static chunks first, then by time.
        chunks.sort_by_key(|span| (!span.is_static, span.time_range.min(), span.chunk.id()));

        let mut prefix_rows = Vec::with_capacity(chunks.len() + 1);
        let mut total = 0;
        prefix_rows.push(0);
        for span in &chunks {
            total += span.num_rows;
            prefix_rows.push(total);
        }

        let num_static_rows = chunks
            .iter()
            .take_while(|span| span.is_static)
            .map(|span| span.num_rows)
            .sum();

        Self {
            timeline: *timeline,
            component,
            level_component,
            filter: filter.cloned(),
            chunks,
            prefix_rows,
            num_static_rows,
            levels,
            any_missing,
        }
    }

    /// All log levels that occur anywhere in the considered chunks.
    pub fn levels(&self) -> &BTreeSet<String> {
        &self.levels
    }

    /// Total number of text log rows (static and temporal).
    pub fn num_rows(&self) -> u64 {
        self.prefix_rows.last().copied().unwrap_or(0)
    }

    pub fn num_static_rows(&self) -> u64 {
        self.num_static_rows
    }

    /// Number of rows strictly before time `t`: all static rows plus temporal rows with time < `t`.
    ///
    /// Equivalently: the global row index of the first temporal row with time >= `t`.
    pub fn rows_before(&self, t: TimeInt) -> u64 {
        re_tracing::profile_function!();

        let mut count = 0;
        for span in &self.chunks {
            if span.is_static {
                // Static rows sort before any temporal time.
                count += span.num_rows;
                continue;
            }
            if span.time_range.min() >= t {
                // Chunks are sorted by min time: no further chunk can contribute.
                break;
            }
            if span.time_range.max() < t {
                count += span.num_rows;
            } else {
                count += count_rows_before_in_chunk(
                    &span.chunk,
                    &self.timeline,
                    self.component,
                    self.level_component,
                    self.filter.as_ref(),
                    t,
                );
            }
        }
        count
    }

    /// The time window covering all chunks that contribute rows in `rows`.
    ///
    /// Returns `None` when no temporal chunk overlaps the given row range.
    /// (Static chunks are excluded: any range query returns them regardless.)
    pub fn time_window_for_rows(&self, rows: Range<u64>) -> Option<(TimeInt, TimeInt)> {
        let mut window: Option<(TimeInt, TimeInt)> = None;

        for (i, span) in self.chunks.iter().enumerate() {
            if self.prefix_rows[i] >= rows.end {
                break;
            }
            if self.prefix_rows[i + 1] <= rows.start || span.is_static {
                continue;
            }

            let (min, max) = window.unwrap_or((TimeInt::MAX, TimeInt::MIN));
            window = Some((
                min.min(span.time_range.min()),
                max.max(span.time_range.max()),
            ));
        }

        window
    }
}

/// Counts the rows in `chunk` that hold a (non-null) `component` value at a time strictly
/// before `t` (and that pass the level filter, if any).
///
/// Handles unsorted chunks (out-of-order logging) correctly by scanning the time column.
fn count_rows_before_in_chunk(
    chunk: &Chunk,
    timeline: &TimelineName,
    component: ComponentIdentifier,
    level_component: ComponentIdentifier,
    filter: Option<&LevelFilter>,
    t: TimeInt,
) -> u64 {
    re_tracing::profile_function!();

    let Some(time_column) = chunk.timelines().get(timeline) else {
        return 0;
    };
    let Some(list_array) = chunk.components().get_array(component) else {
        return 0;
    };

    let t = t.as_i64();
    let times = time_column.times_raw();
    let validity = list_array.nulls();
    let levels = filter.map(|filter| (filter, RowLevels::new(chunk, level_component)));

    (0..times.len())
        .filter(|&row| {
            validity.is_none_or(|validity| validity.is_valid(row))
                && times[row] < t
                && levels
                    .as_ref()
                    .is_none_or(|(filter, levels)| filter.matches(levels.first(row)))
        })
        .count() as u64
}

/// Per-row access to the first log level instance of a chunk.
///
/// This deliberately only looks at the chunk row itself — no blueprint overrides, no
/// latest-at clamping from earlier rows — mirroring how the visualizer builds its entries
/// (exact index join), so that row counts derived here always line up with the materialized
/// entries.
struct RowLevels<'a> {
    list_and_values: Option<(&'a ArrowListArray, &'a ArrowStringArray)>,
}

impl<'a> RowLevels<'a> {
    fn new(chunk: &'a Chunk, level_component: ComponentIdentifier) -> Self {
        let list_and_values = chunk
            .components()
            .get_array(level_component)
            .and_then(|list| {
                let values = list.values().as_any().downcast_ref::<ArrowStringArray>()?;
                Some((list, values))
            });
        Self { list_and_values }
    }

    /// The first level value of the given chunk row, if any.
    fn first(&self, row: usize) -> Option<&'a str> {
        let (list, values) = self.list_and_values?;
        if list.nulls().is_some_and(|validity| !validity.is_valid(row)) {
            return None;
        }
        let start = usize::try_from(list.value_offsets()[row]).ok()?;
        let end = usize::try_from(list.value_offsets()[row + 1]).ok()?;
        (start < end && values.is_valid(start)).then(|| values.value(start))
    }
}

/// Counts, per log level, the rows of `chunk` that hold a (non-null) `component` value.
fn level_counts_for_chunk(
    chunk: &Chunk,
    component: ComponentIdentifier,
    level_component: ComponentIdentifier,
) -> LevelCounts {
    re_tracing::profile_function!();

    let mut counts = LevelCounts::default();

    let Some(list_array) = chunk.components().get_array(component) else {
        return counts;
    };

    let validity = list_array.nulls();
    let levels = RowLevels::new(chunk, level_component);

    for row in 0..list_array.len() {
        if validity.is_some_and(|validity| !validity.is_valid(row)) {
            continue;
        }
        match levels.first(row) {
            Some(level) => {
                if let Some(count) = counts.per_level.get_mut(level) {
                    *count += 1;
                } else {
                    counts.per_level.insert(level.to_owned(), 1);
                }
            }
            None => counts.unleveled += 1,
        }
    }

    counts
}

#[cfg(test)]
mod tests {
    use re_chunk_store::{ChunkStore, ChunkStoreConfig};
    use re_log_types::{StoreId, StoreKind, Timeline};
    use re_sdk_types::archetypes::TextLog;

    use super::*;

    fn store_with_chunks(chunks: impl IntoIterator<Item = Chunk>) -> ChunkStore {
        let mut store = ChunkStore::new(
            StoreId::random(StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );
        for chunk in chunks {
            store.insert_chunk(&Arc::new(chunk)).unwrap();
        }
        store
    }

    fn text_chunk(entity: &str, timeline: Timeline, ticks: impl IntoIterator<Item = i64>) -> Chunk {
        let mut builder = Chunk::builder(entity);
        for tick in ticks {
            builder = builder.with_archetype_auto_row(
                [(timeline, tick)],
                &TextLog::new(format!("{entity} {tick}")),
            );
        }
        builder.build().unwrap()
    }

    fn layout_for(store: &ChunkStore, timeline: &Timeline, entities: &[EntityPath]) -> RowLayout {
        layout_with_filter(store, timeline, entities, None)
    }

    fn layout_with_filter(
        store: &ChunkStore,
        timeline: &Timeline,
        entities: &[EntityPath],
        filter: Option<&LevelFilter>,
    ) -> RowLayout {
        RowLayout::build(
            store,
            timeline.name(),
            TextLog::descriptor_text().component,
            TextLog::descriptor_level().component,
            entities.iter(),
            filter,
            &mut LevelCountCache::default(),
        )
    }

    #[test]
    fn overlapping_chunks_have_exact_counts() {
        let timeline = Timeline::log_tick();
        let store = store_with_chunks([
            text_chunk("a", timeline, [0, 2, 4, 6, 8]),
            text_chunk("b", timeline, [1, 3, 5, 7, 9]),
        ]);
        let layout = layout_for(
            &store,
            &timeline,
            &[EntityPath::from("a"), EntityPath::from("b")],
        );

        assert_eq!(layout.num_rows(), 10);
        assert_eq!(layout.num_static_rows(), 0);

        // Both chunks overlap the whole range, so every boundary requires exact,
        // per-chunk row counting.
        for t in 0..=10 {
            assert_eq!(
                layout.rows_before(TimeInt::new_temporal(t)),
                t as u64,
                "rows_before({t})"
            );
        }
    }

    #[test]
    fn static_rows_sort_first() {
        let timeline = Timeline::log_tick();

        let static_chunk = Chunk::builder("static")
            .with_archetype_auto_row(re_log_types::TimePoint::default(), &TextLog::new("static"))
            .build()
            .unwrap();

        let store = store_with_chunks([static_chunk, text_chunk("a", timeline, [10, 20])]);
        let layout = layout_for(
            &store,
            &timeline,
            &[EntityPath::from("static"), EntityPath::from("a")],
        );

        assert_eq!(layout.num_rows(), 3);
        assert_eq!(layout.num_static_rows(), 1);

        // The static row counts as "before" any temporal time.
        assert_eq!(layout.rows_before(TimeInt::new_temporal(0)), 1);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(15)), 2);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(21)), 3);
    }

    #[test]
    fn time_window_covers_requested_rows() {
        let timeline = Timeline::log_tick();
        let store = store_with_chunks([
            text_chunk("a", timeline, [0, 1, 2]),
            text_chunk("a", timeline, [10, 11, 12]),
            text_chunk("a", timeline, [20, 21, 22]),
        ]);
        let layout = layout_for(&store, &timeline, &[EntityPath::from("a")]);

        assert_eq!(layout.num_rows(), 9);

        // Rows 3..6 live in the middle chunk.
        let (min, max) = layout.time_window_for_rows(3..6).unwrap();
        assert_eq!(min, TimeInt::new_temporal(10));
        assert_eq!(max, TimeInt::new_temporal(12));

        // A range spanning chunk boundaries covers both chunks.
        let (min, max) = layout.time_window_for_rows(2..4).unwrap();
        assert_eq!(min, TimeInt::new_temporal(0));
        assert_eq!(max, TimeInt::new_temporal(12));

        assert!(layout.time_window_for_rows(9..9).is_none());
    }

    #[test]
    fn level_filter_gives_exact_counts() {
        let timeline = Timeline::log_tick();

        // Ticks 0..10; even ticks are INFO, odd ticks are WARN.
        let mut builder = Chunk::builder("a");
        for tick in 0..10 {
            let level = if tick % 2 == 0 { "INFO" } else { "WARN" };
            builder = builder.with_archetype_auto_row(
                [(timeline, tick)],
                &TextLog::new(format!("{tick}")).with_level(level),
            );
        }
        // A second chunk whose rows have no level at all: those always pass the filter.
        let store = store_with_chunks([
            builder.build().unwrap(),
            text_chunk("a", timeline, [100, 101]),
        ]);
        let entities = [EntityPath::from("a")];

        let unfiltered = layout_for(&store, &timeline, &entities);
        assert_eq!(unfiltered.num_rows(), 12);
        assert_eq!(
            unfiltered.levels().iter().cloned().collect::<Vec<_>>(),
            vec!["INFO".to_owned(), "WARN".to_owned()]
        );

        let filter = LevelFilter(std::iter::once("WARN".to_owned()).collect());
        let filtered = layout_with_filter(&store, &timeline, &entities, Some(&filter));

        // 5 WARN rows + 2 unleveled rows.
        assert_eq!(filtered.num_rows(), 7);

        // Boundaries inside the mixed chunk require filtered per-row counting.
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(0)), 0);
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(2)), 1); // WARN@1
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(6)), 3); // WARN@1,3,5
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(50)), 5);
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(101)), 6);

        // A filter matching nothing still shows unleveled rows.
        let none = LevelFilter(BTreeSet::default());
        let none = layout_with_filter(&store, &timeline, &entities, Some(&none));
        assert_eq!(none.num_rows(), 2);
    }
}
