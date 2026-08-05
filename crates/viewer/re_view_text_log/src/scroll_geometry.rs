use std::ops::Range;
use std::sync::Arc;

use re_chunk_store::external::arrow::array::Array as _;
use re_chunk_store::{
    AbsoluteTimeRange, Chunk, ChunkStore, ChunkTrackingMode, RangeQuery, TimeInt,
};
use re_entity_db::EntityPath;
use re_log_types::TimelineName;
use re_sdk_types::ComponentIdentifier;

/// Chunk-level metadata for one chunk holding text log rows.
struct ChunkGeom {
    /// Time range covered by the chunk on the geometry's timeline.
    ///
    /// Meaningless for static chunks ([`AbsoluteTimeRange`] cannot represent static times).
    time_range: AbsoluteTimeRange,

    /// Number of text log rows in this chunk.
    num_rows: u64,

    is_static: bool,

    chunk: Arc<Chunk>,
}

/// Row layout of the text log table, derived from chunk-level metadata only.
///
/// This is what allows the view to know how many rows there are — and roughly where in time
/// each row lives — without touching any row data:
/// row counts come from the per-chunk validity bitmaps and are exact,
/// and only locating a time window boundary *inside* a chunk requires peeking at that
/// (already loaded) chunk's time column.
///
/// Rows are ordered with static rows first, then by time.
/// Where chunks of different entities overlap in time, the layout is approximate at chunk
/// granularity, but stable; the visible window itself is always rendered in exact time order.
pub struct ScrollGeometry {
    timeline: TimelineName,
    component: ComponentIdentifier,

    /// Sorted by `time_range.min` (which puts static chunks first).
    chunks: Vec<ChunkGeom>,

    /// Prefix sums of `chunks[..i].num_rows`; length is `chunks.len() + 1`.
    prefix_rows: Vec<u64>,

    num_static_rows: u64,

    /// Chunks that the store knows of but that aren't loaded; row counts are incomplete.
    //
    // TODO(#7562): once this is fed from the `RrdManifestIndex`, unloaded chunks can
    // contribute their row counts too, making the layout complete in streaming scenarios.
    pub any_missing: bool,
}

impl ScrollGeometry {
    pub fn build<'a>(
        store: &ChunkStore,
        timeline: &TimelineName,
        component: ComponentIdentifier,
        entities: impl Iterator<Item = &'a EntityPath>,
    ) -> Self {
        re_tracing::profile_function!();

        let query = RangeQuery::new(*timeline, AbsoluteTimeRange::EVERYTHING);

        let mut chunks = Vec::new();
        let mut any_missing = false;

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
                let Some(num_rows) = chunk.num_events_for_component(component) else {
                    continue;
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

                chunks.push(ChunkGeom {
                    time_range,
                    num_rows,
                    is_static,
                    chunk,
                });
            }
        }

        // Static chunks first, then by time.
        chunks.sort_by_key(|geom| (!geom.is_static, geom.time_range.min(), geom.chunk.id()));

        let mut prefix_rows = Vec::with_capacity(chunks.len() + 1);
        let mut total = 0;
        prefix_rows.push(0);
        for geom in &chunks {
            total += geom.num_rows;
            prefix_rows.push(total);
        }

        let num_static_rows = chunks
            .iter()
            .take_while(|geom| geom.is_static)
            .map(|geom| geom.num_rows)
            .sum();

        Self {
            timeline: *timeline,
            component,
            chunks,
            prefix_rows,
            num_static_rows,
            any_missing,
        }
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
        for geom in &self.chunks {
            if geom.is_static {
                // Static rows sort before any temporal time.
                count += geom.num_rows;
                continue;
            }
            if geom.time_range.min() >= t {
                // Chunks are sorted by min time: no further chunk can contribute.
                break;
            }
            if geom.time_range.max() < t {
                count += geom.num_rows;
            } else {
                count += count_rows_before_in_chunk(&geom.chunk, &self.timeline, self.component, t);
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

        for (i, geom) in self.chunks.iter().enumerate() {
            if self.prefix_rows[i] >= rows.end {
                break;
            }
            if self.prefix_rows[i + 1] <= rows.start || geom.is_static {
                continue;
            }

            let (min, max) = window.unwrap_or((TimeInt::MAX, TimeInt::MIN));
            window = Some((
                min.min(geom.time_range.min()),
                max.max(geom.time_range.max()),
            ));
        }

        window
    }
}

/// Counts the rows in `chunk` that hold a (non-null) `component` value at a time strictly
/// before `t`.
///
/// Handles unsorted chunks (out-of-order logging) correctly by scanning the time column.
fn count_rows_before_in_chunk(
    chunk: &Chunk,
    timeline: &TimelineName,
    component: ComponentIdentifier,
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

    if let Some(validity) = list_array.nulls() {
        times
            .iter()
            .zip(validity.iter())
            .filter(|&(&time, valid)| valid && time < t)
            .count() as u64
    } else {
        times.iter().filter(|&&time| time < t).count() as u64
    }
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

    fn geometry_for(
        store: &ChunkStore,
        timeline: &Timeline,
        entities: &[EntityPath],
    ) -> ScrollGeometry {
        ScrollGeometry::build(
            store,
            timeline.name(),
            TextLog::descriptor_text().component,
            entities.iter(),
        )
    }

    #[test]
    fn overlapping_chunks_have_exact_counts() {
        let timeline = Timeline::log_tick();
        let store = store_with_chunks([
            text_chunk("a", timeline, [0, 2, 4, 6, 8]),
            text_chunk("b", timeline, [1, 3, 5, 7, 9]),
        ]);
        let geometry = geometry_for(
            &store,
            &timeline,
            &[EntityPath::from("a"), EntityPath::from("b")],
        );

        assert_eq!(geometry.num_rows(), 10);
        assert_eq!(geometry.num_static_rows(), 0);

        // Both chunks overlap the whole range, so every boundary requires exact,
        // per-chunk row counting.
        for t in 0..=10 {
            assert_eq!(
                geometry.rows_before(TimeInt::new_temporal(t)),
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
        let geometry = geometry_for(
            &store,
            &timeline,
            &[EntityPath::from("static"), EntityPath::from("a")],
        );

        assert_eq!(geometry.num_rows(), 3);
        assert_eq!(geometry.num_static_rows(), 1);

        // The static row counts as "before" any temporal time.
        assert_eq!(geometry.rows_before(TimeInt::new_temporal(0)), 1);
        assert_eq!(geometry.rows_before(TimeInt::new_temporal(15)), 2);
        assert_eq!(geometry.rows_before(TimeInt::new_temporal(21)), 3);
    }

    #[test]
    fn time_window_covers_requested_rows() {
        let timeline = Timeline::log_tick();
        let store = store_with_chunks([
            text_chunk("a", timeline, [0, 1, 2]),
            text_chunk("a", timeline, [10, 11, 12]),
            text_chunk("a", timeline, [20, 21, 22]),
        ]);
        let geometry = geometry_for(&store, &timeline, &[EntityPath::from("a")]);

        assert_eq!(geometry.num_rows(), 9);

        // Rows 3..6 live in the middle chunk.
        let (min, max) = geometry.time_window_for_rows(3..6).unwrap();
        assert_eq!(min, TimeInt::new_temporal(10));
        assert_eq!(max, TimeInt::new_temporal(12));

        // A range spanning chunk boundaries covers both chunks.
        let (min, max) = geometry.time_window_for_rows(2..4).unwrap();
        assert_eq!(min, TimeInt::new_temporal(0));
        assert_eq!(max, TimeInt::new_temporal(12));

        assert!(geometry.time_window_for_rows(9..9).is_none());
    }
}
