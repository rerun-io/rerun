use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;

use re_chunk_store::external::arrow::array::{
    Array as _, ListArray as ArrowListArray, StringArray as ArrowStringArray,
    UInt32Array as ArrowUInt32Array,
};
use re_chunk_store::{AbsoluteTimeRange, Chunk, ChunkId, ChunkStoreEvent, TimeInt};
use re_entity_db::EntityPath;
use re_log_types::TimelineName;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::Color;
use re_viewer_context::Cache;

/// Blueprint overrides and view defaults for an entity's text log rows.
///
/// Both are per-entity constants (a blueprint override/default is a single latest-at value),
/// which is what keeps them compatible with the metadata-derived row layout: they never
/// require per-row joins. A row's effective level/color is resolved as
/// override > the row's own value > view default.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RowOverrides {
    pub level_override: Option<String>,
    pub level_default: Option<String>,
    pub color_override: Option<Color>,
    pub color_default: Option<Color>,
}

impl RowOverrides {
    /// The level a row effectively has: override > the row's own level > default.
    fn effective_level<'a>(&'a self, row_level: Option<&'a str>) -> Option<&'a str> {
        self.level_override
            .as_deref()
            .or(row_level)
            .or(self.level_default.as_deref())
    }
}

/// An explicit log level filter: only rows whose (effective) level is in the set are shown.
///
/// Rows without any level always pass.
#[derive(Clone, PartialEq, Eq)]
pub struct LevelFilter(pub BTreeSet<String>);

impl LevelFilter {
    pub fn matches(&self, level: Option<&str>) -> bool {
        level.is_none_or(|lvl| self.0.contains(lvl))
    }
}

/// Per-level row counts for a single chunk.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LevelCounts {
    /// Number of text log rows whose (first) level value is the given string.
    per_level: BTreeMap<String, u64>,

    /// Number of text log rows without a level value.
    unleveled: u64,
}

impl LevelCounts {
    fn total(&self) -> u64 {
        self.unleveled + self.per_level.values().sum::<u64>()
    }

    fn matching(&self, filter: &LevelFilter, overrides: &RowOverrides) -> u64 {
        // An override applies to every row alike: all or nothing.
        if let Some(lvl) = &overrides.level_override {
            return if filter.0.contains(lvl) {
                self.total()
            } else {
                0
            };
        }

        // A default only applies to rows without their own level.
        let unleveled = match &overrides.level_default {
            Some(default) if !filter.0.contains(default) => 0,
            _ => self.unleveled,
        };

        unleveled
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

/// Per-chunk [`LevelCounts`], computed once per chunk and shared by all text log views.
///
/// Chunks are immutable, so an entry stays valid until the chunk is evicted from the store
/// (deleted or compacted away), which is what makes level-filtered row counts (and the set
/// of levels offered by the filter UI) as cheap as unfiltered ones after a chunk's first frame.
#[derive(Default)]
pub struct LevelCountCache(BTreeMap<ChunkId, LevelCounts>);

impl LevelCountCache {
    fn counts_for(
        &mut self,
        chunk: &Chunk,
        component: ComponentIdentifier,
        level_component: ComponentIdentifier,
    ) -> &LevelCounts {
        self.0
            .entry(chunk.id())
            .or_insert_with(|| level_counts_for_chunk(chunk, component, level_component))
    }
}

impl Cache for LevelCountCache {
    fn name(&self) -> &'static str {
        "TextLogLevelCountCache"
    }

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn on_store_events(
        &mut self,
        events: &[&ChunkStoreEvent],
        _entity_db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();

        let deleted: BTreeSet<ChunkId> = events
            .iter()
            .filter_map(|event| event.to_deletion())
            .map(|deletion| deletion.chunk.id())
            .collect();
        if deleted.is_empty() {
            return;
        }

        self.0.retain(|chunk_id, _| !deleted.contains(chunk_id));
    }
}

impl re_byte_size::MemUsageTreeCapture for LevelCountCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(re_byte_size::SizeBytes::total_size_bytes(&self.0))
    }
}

/// The contiguous span of table rows contributed by one chunk.
#[derive(Clone)]
struct ChunkSpan {
    /// Time range covered by the chunk on the layout's timeline.
    ///
    /// Meaningless for static chunks ([`AbsoluteTimeRange`] cannot represent static times).
    time_range: AbsoluteTimeRange,

    /// Number of table rows this chunk contributes (only filter-matching rows count).
    num_rows: u64,

    is_static: bool,

    chunk: Chunk,

    /// Blueprint overrides/defaults of the chunk's entity.
    overrides: RowOverrides,
}

/// Row layout of the text log table, derived from chunk-level metadata only.
///
/// This is what allows the view to know how many rows there are without touching any row data:
/// row counts come from the per-chunk validity bitmaps (or, with a level filter active, from
/// cached per-chunk level counts) and are exact, and only locating a time boundary *inside* a
/// chunk requires looking at that chunk's time column.
///
/// Rows are ordered with static rows first, then by time.
/// Where chunks of different entities overlap in time, the layout is approximate at chunk
/// granularity, but stable since the visible window itself is always rendered in exact time
/// order (see [`Self::visible_rows`]).
///
/// Owns the (cheaply cloned, arrow-backed) chunks it was built from, so resolving visible
/// rows and reading their data needs no further store access.
#[derive(Clone)]
pub struct RowLayout {
    timeline: TimelineName,
    component: ComponentIdentifier,
    level_component: ComponentIdentifier,
    color_component: ComponentIdentifier,

    /// The active level filter; when set, all row counts only cover matching rows.
    filter: Option<LevelFilter>,

    /// Sorted by `time_range.min` (which puts static chunks first).
    chunks: Vec<ChunkSpan>,

    /// Prefix sums of `chunks[..i].num_rows`; length is `chunks.len() + 1`.
    prefix_rows: Vec<u64>,

    num_static_rows: u64,

    /// All log levels that occur anywhere in the considered chunks.
    levels: BTreeSet<String>,
}

impl RowLayout {
    /// Build a row layout for the given text log chunks, optionally applying a level filter.
    ///
    /// The chunks are expected to come from a range query for `component`
    /// (see `TextLogSystem::execute`), i.e. every row holds a text value.
    ///
    /// `overrides` carries the per-entity blueprint overrides and view defaults for
    /// level/color; the filter counts rows by their *effective* level, so counts and
    /// displayed rows always agree.
    pub fn build(
        chunks: Vec<Chunk>,
        timeline: &TimelineName,
        component: ComponentIdentifier,
        level_component: ComponentIdentifier,
        color_component: ComponentIdentifier,
        filter: Option<&LevelFilter>,
        overrides: &HashMap<EntityPath, RowOverrides>,
        level_counts: &mut LevelCountCache,
    ) -> Self {
        re_tracing::profile_function!();

        let mut spans = Vec::new();
        let mut levels = BTreeSet::new();

        for chunk in chunks {
            let Some(total_rows) = chunk.num_events_for_component(component) else {
                continue;
            };
            if total_rows == 0 {
                continue;
            }

            let chunk_overrides = overrides
                .get(chunk.entity_path())
                .cloned()
                .unwrap_or_default();

            // One-time scan per chunk (chunks are immutable, so the counts never go stale);
            // this is what makes level-filtered row counts viable.
            let counts = level_counts.counts_for(&chunk, component, level_component);

            // Offer exactly the levels that can show up in the table.
            if let Some(lvl) = &chunk_overrides.level_override {
                levels.insert(lvl.clone());
            } else {
                levels.extend(counts.per_level.keys().cloned());
                if counts.unleveled > 0
                    && let Some(default) = &chunk_overrides.level_default
                {
                    levels.insert(default.clone());
                }
            }

            let num_rows = match filter {
                Some(filter) => counts.matching(filter, &chunk_overrides),
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

            spans.push(ChunkSpan {
                time_range,
                num_rows,
                is_static,
                chunk,
                overrides: chunk_overrides,
            });
        }

        // Static chunks first, then by time.
        spans.sort_by_key(|span| (!span.is_static, span.time_range.min(), span.chunk.id()));

        let mut prefix_rows = Vec::with_capacity(spans.len() + 1);
        let mut total = 0;
        prefix_rows.push(0);
        for span in &spans {
            total += span.num_rows;
            prefix_rows.push(total);
        }

        let num_static_rows = spans
            .iter()
            .take_while(|span| span.is_static)
            .map(|span| span.num_rows)
            .sum();

        Self {
            timeline: *timeline,
            component,
            level_component,
            color_component,
            filter: filter.cloned(),
            chunks: spans,
            prefix_rows,
            num_static_rows,
            levels,
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
                count += self.count_rows_before_in_chunk(span, t);
            }
        }
        count
    }

    /// Resolves the rows of the table that are currently on screen.
    ///
    /// This is the only place where actual row data (time and level columns) is touched, and
    /// it only looks inside the chunks overlapping the visible time window.
    pub fn visible_rows(&self, rows: Range<u64>) -> VisibleRows {
        re_tracing::profile_function!();

        // Static rows sit at the top of the table; there are usually only a handful,
        // so resolve all of them whenever any is visible.
        let mut static_rows = Vec::new();
        if rows.start < self.num_static_rows {
            for (span_idx, span) in self.chunks.iter().enumerate() {
                if !span.is_static {
                    break;
                }
                self.collect_span_rows(span_idx, span, None, &mut static_rows);
            }
        }

        // For the temporal part, resolve *all* rows within the time window covered by the
        // visible rows' chunks: that guarantees exact global row indices (offset by the exact
        // number of rows before the window), even where chunks of different entities overlap.
        let temporal = rows.start.max(self.num_static_rows)..rows.end.max(self.num_static_rows);
        let mut temporal_offset = 0;
        let mut temporal_rows = Vec::new();
        if temporal.start < temporal.end
            && let Some((min, max)) = self.time_window_for_rows(temporal)
        {
            temporal_offset = self.rows_before(min);

            for (span_idx, span) in self.chunks.iter().enumerate() {
                if span.is_static {
                    continue;
                }
                if span.time_range.min() > max {
                    break;
                }
                if span.time_range.max() < min {
                    continue;
                }
                self.collect_span_rows(span_idx, span, Some((min, max)), &mut temporal_rows);
            }

            // Sort is stable, so rows with equal times stay in chunk order,
            // consistent with how `rows_before` counts them.
            temporal_rows.sort_by_key(|row| row.time);
        }

        VisibleRows {
            num_static_rows: self.num_static_rows,
            static_rows,
            temporal_offset,
            temporal_rows,
        }
    }

    /// The data of a single row: the underlying chunk row, with the entity's blueprint
    /// overrides/defaults applied.
    pub fn row_data(&self, row: &RowRef) -> RowData<'_> {
        let span = &self.chunks[row.span_idx];
        let chunk = &span.chunk;
        let overrides = &span.overrides;

        let row_level = RowStrings::new(chunk, self.level_component).first(row.row);
        let row_color = row_color(chunk, self.color_component, row.row).map(Color::from);

        RowData {
            entity_path: chunk.entity_path(),
            body: RowStrings::new(chunk, self.component).first(row.row),
            level: overrides.effective_level(row_level),
            color: overrides
                .color_override
                .or(row_color)
                .or(overrides.color_default),
        }
    }

    /// The time of a single row on the given timeline, or [`TimeInt::STATIC`] if the row's
    /// chunk has no data on it.
    pub fn row_time(&self, row: &RowRef, timeline: &TimelineName) -> TimeInt {
        let chunk = &self.chunks[row.span_idx].chunk;
        chunk
            .timelines()
            .get(timeline)
            .map_or(TimeInt::STATIC, |time_column| {
                TimeInt::new_temporal(time_column.times_raw()[row.row])
            })
    }

    /// Pushes a [`RowRef`] for every filter-matching row of the span, in row order,
    /// optionally restricted to an (inclusive) time range.
    fn collect_span_rows(
        &self,
        span_idx: usize,
        span: &ChunkSpan,
        time_bounds: Option<(TimeInt, TimeInt)>,
        out: &mut Vec<RowRef>,
    ) {
        let Some(list_array) = span.chunk.components().get_array(self.component) else {
            return;
        };

        let times = span
            .chunk
            .timelines()
            .get(&self.timeline)
            .map(|time_column| time_column.times_raw());

        let validity = list_array.nulls();
        let levels = self
            .filter
            .as_ref()
            .map(|filter| (filter, RowStrings::new(&span.chunk, self.level_component)));

        for row in 0..list_array.len() {
            if validity.is_some_and(|validity| !validity.is_valid(row)) {
                continue;
            }

            let time = times.map_or(TimeInt::STATIC, |times| TimeInt::new_temporal(times[row]));
            if let Some((min, max)) = time_bounds
                && !(min <= time && time <= max)
            {
                continue;
            }

            if levels.as_ref().is_some_and(|(filter, levels)| {
                !filter.matches(span.overrides.effective_level(levels.first(row)))
            }) {
                continue;
            }

            out.push(RowRef {
                span_idx,
                row,
                time,
            });
        }
    }

    /// The time window covering all chunks that contribute rows in `rows`.
    ///
    /// Returns `None` when no temporal chunk overlaps the given row range.
    fn time_window_for_rows(&self, rows: Range<u64>) -> Option<(TimeInt, TimeInt)> {
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

    /// Counts the rows of the span that hold a (non-null) text value at a time strictly
    /// before `t` (and that pass the level filter, if any).
    ///
    /// Handles unsorted chunks (out-of-order logging) correctly by scanning the time column.
    fn count_rows_before_in_chunk(&self, span: &ChunkSpan, t: TimeInt) -> u64 {
        re_tracing::profile_function!();

        let Some(time_column) = span.chunk.timelines().get(&self.timeline) else {
            return 0;
        };
        let Some(list_array) = span.chunk.components().get_array(self.component) else {
            return 0;
        };

        let t = t.as_i64();
        let times = time_column.times_raw();
        let validity = list_array.nulls();
        let levels = self
            .filter
            .as_ref()
            .map(|filter| (filter, RowStrings::new(&span.chunk, self.level_component)));

        (0..times.len())
            .filter(|&row| {
                validity.is_none_or(|validity| validity.is_valid(row))
                    && times[row] < t
                    && levels.as_ref().is_none_or(|(filter, levels)| {
                        filter.matches(span.overrides.effective_level(levels.first(row)))
                    })
            })
            .count() as u64
    }
}

/// Reference to a single table row inside a [`RowLayout`]'s chunks.
pub struct RowRef {
    span_idx: usize,
    row: usize,

    /// Time on the layout's timeline; [`TimeInt::STATIC`] for static rows.
    pub time: TimeInt,
}

/// The currently visible table rows, resolved to exact positions inside the chunks.
///
/// Global row indices map to [`RowRef`]s: static rows sit at the very top, temporal rows
/// follow in exact time order starting at `temporal_offset`.
#[derive(Default)]
pub struct VisibleRows {
    num_static_rows: u64,

    /// All static rows (only resolved when any of them is visible); global rows `0..len`.
    static_rows: Vec<RowRef>,

    /// Global row index of `temporal_rows[0]`.
    temporal_offset: u64,

    /// All rows within the visible chunks' time window, sorted by time.
    temporal_rows: Vec<RowRef>,
}

impl VisibleRows {
    pub fn get(&self, row_nr: u64) -> Option<&RowRef> {
        if row_nr < self.num_static_rows {
            self.static_rows.get(usize::try_from(row_nr).ok()?)
        } else {
            let idx = row_nr.checked_sub(self.temporal_offset)?;
            self.temporal_rows.get(usize::try_from(idx).ok()?)
        }
    }
}

/// The data of a single text log row, borrowed from its chunk (with the entity's blueprint
/// overrides/defaults applied to level and color).
pub struct RowData<'a> {
    pub entity_path: &'a EntityPath,
    pub body: Option<&'a str>,
    pub level: Option<&'a str>,
    pub color: Option<Color>,
}

/// Per-row access to the first instance of a string component of a chunk.
///
/// This deliberately only looks at the chunk row itself: a text log row's data comes from
/// that row alone, so that displayed values always line up with the metadata-derived layout.
struct RowStrings<'a> {
    list_and_values: Option<(&'a ArrowListArray, &'a ArrowStringArray)>,
}

impl<'a> RowStrings<'a> {
    fn new(chunk: &'a Chunk, component: ComponentIdentifier) -> Self {
        let list_and_values = chunk.components().get_array(component).and_then(|list| {
            let values = list.values().as_any().downcast_ref::<ArrowStringArray>()?;
            Some((list, values))
        });
        Self { list_and_values }
    }

    /// The first value of the given chunk row, if any.
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

/// The first color instance of the given chunk row, if any.
fn row_color(chunk: &Chunk, color_component: ComponentIdentifier, row: usize) -> Option<u32> {
    let list = chunk.components().get_array(color_component)?;
    if list.nulls().is_some_and(|validity| !validity.is_valid(row)) {
        return None;
    }
    let values = list.values().as_any().downcast_ref::<ArrowUInt32Array>()?;
    let start = usize::try_from(list.value_offsets()[row]).ok()?;
    let end = usize::try_from(list.value_offsets()[row + 1]).ok()?;
    (start < end && values.is_valid(start)).then(|| values.value(start))
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
    let levels = RowStrings::new(chunk, level_component);

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
    use re_log_types::Timeline;
    use re_sdk_types::archetypes::TextLog;

    use super::*;

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

    fn layout_for(chunks: &[Chunk], timeline: &Timeline) -> RowLayout {
        layout_with_filter(chunks, timeline, None)
    }

    fn layout_with_filter(
        chunks: &[Chunk],
        timeline: &Timeline,
        filter: Option<&LevelFilter>,
    ) -> RowLayout {
        layout_with_overrides(chunks, timeline, filter, &HashMap::default())
    }

    fn layout_with_overrides(
        chunks: &[Chunk],
        timeline: &Timeline,
        filter: Option<&LevelFilter>,
        overrides: &HashMap<EntityPath, RowOverrides>,
    ) -> RowLayout {
        RowLayout::build(
            chunks.to_vec(),
            timeline.name(),
            TextLog::descriptor_text().component,
            TextLog::descriptor_level().component,
            TextLog::descriptor_color().component,
            filter,
            overrides,
            &mut LevelCountCache::default(),
        )
    }

    #[test]
    fn overlapping_chunks_have_exact_counts() {
        let timeline = Timeline::log_tick();
        let chunks = [
            text_chunk("a", timeline, [0, 2, 4, 6, 8]),
            text_chunk("b", timeline, [1, 3, 5, 7, 9]),
        ];
        let layout = layout_for(&chunks, &timeline);

        assert_eq!(layout.num_rows(), 10);
        assert_eq!(layout.rows_before(TimeInt::MIN), 0); // no static rows

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
    fn visible_rows_interleave_overlapping_chunks_in_time_order() {
        let timeline = Timeline::log_tick();
        let chunks = [
            text_chunk("a", timeline, [0, 2, 4, 6, 8]),
            text_chunk("b", timeline, [1, 3, 5, 7, 9]),
        ];
        let layout = layout_for(&chunks, &timeline);

        // Any visible sub-range resolves to globally time-ordered rows.
        let visible = layout.visible_rows(3..7);
        for row_nr in 3_u64..7 {
            let row = visible.get(row_nr).unwrap();
            assert_eq!(
                row.time,
                TimeInt::new_temporal(i64::try_from(row_nr).unwrap())
            );
            let data = layout.row_data(row);
            assert_eq!(
                data.body.unwrap(),
                format!("{} {row_nr}", if row_nr % 2 == 0 { "a" } else { "b" })
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

        let chunks = [static_chunk, text_chunk("a", timeline, [10, 20])];
        let layout = layout_for(&chunks, &timeline);

        assert_eq!(layout.num_rows(), 3);
        assert_eq!(layout.rows_before(TimeInt::MIN), 1); // just the static row

        // The static row counts as "before" any temporal time.
        assert_eq!(layout.rows_before(TimeInt::new_temporal(0)), 1);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(15)), 2);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(21)), 3);

        // The static row resolves with a static time; temporal rows follow.
        let visible = layout.visible_rows(0..3);
        assert_eq!(visible.get(0).unwrap().time, TimeInt::STATIC);
        assert_eq!(visible.get(1).unwrap().time, TimeInt::new_temporal(10));
        assert_eq!(visible.get(2).unwrap().time, TimeInt::new_temporal(20));
    }

    #[test]
    fn visible_rows_cover_requested_range() {
        let timeline = Timeline::log_tick();
        let chunks = [
            text_chunk("a", timeline, [0, 1, 2]),
            text_chunk("a", timeline, [10, 11, 12]),
            text_chunk("a", timeline, [20, 21, 22]),
        ];
        let layout = layout_for(&chunks, &timeline);

        assert_eq!(layout.num_rows(), 9);

        // Rows 3..6 live in the middle chunk.
        let visible = layout.visible_rows(3..6);
        assert_eq!(visible.get(3).unwrap().time, TimeInt::new_temporal(10));
        assert_eq!(visible.get(5).unwrap().time, TimeInt::new_temporal(12));

        // A range spanning chunk boundaries covers both chunks.
        let visible = layout.visible_rows(2..4);
        assert_eq!(visible.get(2).unwrap().time, TimeInt::new_temporal(2));
        assert_eq!(visible.get(3).unwrap().time, TimeInt::new_temporal(10));

        assert!(layout.visible_rows(9..9).get(9).is_none());
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
        let chunks = [
            builder.build().unwrap(),
            text_chunk("a", timeline, [100, 101]),
        ];

        let unfiltered = layout_for(&chunks, &timeline);
        assert_eq!(unfiltered.num_rows(), 12);
        assert_eq!(
            unfiltered.levels().iter().cloned().collect::<Vec<_>>(),
            vec!["INFO".to_owned(), "WARN".to_owned()]
        );

        let filter = LevelFilter(std::iter::once("WARN".to_owned()).collect());
        let filtered = layout_with_filter(&chunks, &timeline, Some(&filter));

        // 5 WARN rows + 2 unleveled rows.
        assert_eq!(filtered.num_rows(), 7);

        // Boundaries inside the mixed chunk require filtered per-row counting.
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(0)), 0);
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(2)), 1); // WARN@1
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(6)), 3); // WARN@1,3,5
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(50)), 5);
        assert_eq!(filtered.rows_before(TimeInt::new_temporal(101)), 6);

        // The resolved rows skip filtered-out ones and line up with the counts.
        let visible = filtered.visible_rows(0..7);
        let times: Vec<_> = (0..7)
            .map(|row_nr| visible.get(row_nr).unwrap().time.as_i64())
            .collect();
        assert_eq!(times, vec![1, 3, 5, 7, 9, 100, 101]);

        // A filter matching nothing still shows unleveled rows.
        let none = LevelFilter(BTreeSet::default());
        let none = layout_with_filter(&chunks, &timeline, Some(&none));
        assert_eq!(none.num_rows(), 2);
    }

    #[test]
    fn level_counts_survive_chunks_excluded_by_the_filter() {
        let timeline = Timeline::log_tick();

        let mut builder = Chunk::builder("a");
        for tick in 0..10 {
            builder = builder.with_archetype_auto_row(
                [(timeline, tick)],
                &TextLog::new(format!("{tick}")).with_level("INFO"),
            );
        }
        let chunks = [builder.build().unwrap()];

        // The filter excludes the whole chunk, so it contributes no span…
        let mut cache = LevelCountCache::default();
        let filter = LevelFilter(std::iter::once("WARN".to_owned()).collect());
        let layout = RowLayout::build(
            chunks.to_vec(),
            timeline.name(),
            TextLog::descriptor_text().component,
            TextLog::descriptor_level().component,
            TextLog::descriptor_color().component,
            Some(&filter),
            &HashMap::default(),
            &mut cache,
        );
        assert_eq!(layout.num_rows(), 0);

        // …but its cached counts must survive (eviction only happens on chunk deletion),
        // or it would be rescanned every frame.
        assert_eq!(cache.0.len(), 1);
    }

    #[test]
    fn level_override_applies_to_all_rows() {
        let timeline = Timeline::log_tick();

        // Rows logged as INFO, but the blueprint overrides the entity's level to WARN.
        let mut builder = Chunk::builder("a");
        for tick in 0..4 {
            builder = builder.with_archetype_auto_row(
                [(timeline, tick)],
                &TextLog::new(format!("{tick}")).with_level("INFO"),
            );
        }
        let chunks = [builder.build().unwrap()];
        let overrides = HashMap::from([(
            EntityPath::from("a"),
            RowOverrides {
                level_override: Some("WARN".to_owned()),
                color_override: Some(Color::from_rgb(255, 0, 0)),
                ..Default::default()
            },
        )]);

        // The override wins over the rows' own levels, in counts and in row data alike.
        let filter = LevelFilter(std::iter::once("INFO".to_owned()).collect());
        let layout = layout_with_overrides(&chunks, &timeline, Some(&filter), &overrides);
        assert_eq!(layout.num_rows(), 0);

        let filter = LevelFilter(std::iter::once("WARN".to_owned()).collect());
        let layout = layout_with_overrides(&chunks, &timeline, Some(&filter), &overrides);
        assert_eq!(layout.num_rows(), 4);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(2)), 2);

        // The filter UI only offers the level that actually shows.
        assert_eq!(
            layout.levels().iter().cloned().collect::<Vec<_>>(),
            vec!["WARN".to_owned()]
        );

        let visible = layout.visible_rows(0..4);
        let data = layout.row_data(visible.get(0).unwrap());
        assert_eq!(data.level, Some("WARN"));
        assert_eq!(data.color, Some(Color::from_rgb(255, 0, 0)));
    }

    #[test]
    fn level_default_applies_to_unleveled_rows_only() {
        let timeline = Timeline::log_tick();

        // Rows 0/2 are INFO; rows 1/3 have no level and fall back to the view default.
        let mut builder = Chunk::builder("a");
        for tick in 0..4 {
            let mut row = TextLog::new(format!("{tick}"));
            if tick % 2 == 0 {
                row = row.with_level("INFO");
            }
            builder = builder.with_archetype_auto_row([(timeline, tick)], &row);
        }
        let chunks = [builder.build().unwrap()];
        let overrides = HashMap::from([(
            EntityPath::from("a"),
            RowOverrides {
                level_default: Some("DEBUG".to_owned()),
                ..Default::default()
            },
        )]);

        // Unleveled rows now count as DEBUG: they no longer pass every filter.
        let filter = LevelFilter(std::iter::once("INFO".to_owned()).collect());
        let layout = layout_with_overrides(&chunks, &timeline, Some(&filter), &overrides);
        assert_eq!(layout.num_rows(), 2);

        let filter = LevelFilter(std::iter::once("DEBUG".to_owned()).collect());
        let layout = layout_with_overrides(&chunks, &timeline, Some(&filter), &overrides);
        assert_eq!(layout.num_rows(), 2);
        assert_eq!(layout.rows_before(TimeInt::new_temporal(3)), 1); // DEBUG@1

        // Both the logged and the default level are offered by the filter UI.
        let unfiltered = layout_with_overrides(&chunks, &timeline, None, &overrides);
        assert_eq!(
            unfiltered.levels().iter().cloned().collect::<Vec<_>>(),
            vec!["DEBUG".to_owned(), "INFO".to_owned()]
        );

        let visible = unfiltered.visible_rows(0..4);
        assert_eq!(
            unfiltered.row_data(visible.get(0).unwrap()).level,
            Some("INFO")
        );
        assert_eq!(
            unfiltered.row_data(visible.get(1).unwrap()).level,
            Some("DEBUG")
        );
    }
}
