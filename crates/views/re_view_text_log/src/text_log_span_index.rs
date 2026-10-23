use std::collections::BTreeMap;
use std::sync::Arc;

use re_chunk_store::AbsoluteTimeRange;
use re_log_types::TimeInt;

use crate::text_log_index_cache::{IndexedRow, TextLogIndex};

/// Estimated rows per unresolved span, bounding the payload loaded by one span query.
///
/// Coverage is split into spans of at most this size, and adjacent unresolved spans are
/// merged while their combined count stays within it.
/// Equal timestamps cannot be split, so a single-timestamp span may exceed it.
pub const TARGET_SPAN_ROWS: usize =
    (re_chunk_store::ChunkStoreConfig::DEFAULT.chunk_max_rows / 2) as usize;

/// Navigation spans with estimated counts until queries resolve their filtered text instances.
///
/// Spans are disjoint and ordered.
/// Span identities remain stable as counts change. Retained data consists only of counts
/// and row identities, never chunks or log contents.
#[derive(Clone, Default, re_byte_size::SizeBytes)]
pub struct TextLogSpanIndex {
    spans: Vec<TextLogNavigationSpan>,
}

/// A scroll position relative to a span in one span layout.
/// Remains usable as span counts are refined, but must not be reused with a different index key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
pub struct TextLogSpanAnchor {
    /// Position of the span in the index; refinement does not reorder spans.
    span_id: usize,

    /// Position within the span, not a recording row ID; clamped when the span shrinks.
    local_row: usize,
}

#[derive(Clone, re_byte_size::SizeBytes)]
struct TextLogNavigationSpan {
    range: AbsoluteTimeRange,
    count: usize,
    index: Option<Arc<TextLogIndex>>,
}

impl TextLogSpanIndex {
    /// Builds disjoint spans from potentially overlapping component ranges and counts.
    /// Input counts describe component rows, not text instances or level-filtered table rows;
    /// uniform density within each range supplies initial navigation estimates.
    pub fn new(ranges: impl IntoIterator<Item = (AbsoluteTimeRange, u64)>) -> Self {
        re_tracing::profile_function!();

        let mut spans = disjoint_densities(ranges)
            .into_iter()
            .flat_map(|(range, density)| split_spans(range, density))
            .collect();
        coalesce(&mut spans);
        Self { spans }
    }

    /// Reconciles candidate coverage, preserving resolved spans and their identities.
    /// Unresolved spans may be merged with unresolved neighbors, so the resulting partition
    /// does not depend on how store changes were batched.
    /// Callers must include every changed text range and each level update's carry-forward
    /// influence in `dirty`. A true return value invalidates old span-relative anchors.
    pub fn reconcile(
        &mut self,
        ranges: impl IntoIterator<Item = (AbsoluteTimeRange, u64)>,
        dirty: &[AbsoluteTimeRange],
    ) -> bool {
        re_tracing::profile_function!();
        let mut invalidated = vec![false; self.spans.len()];
        for range in dirty.iter().filter(|range| !range.is_empty()) {
            let start = self
                .spans
                .partition_point(|span| span.range.max() < range.min());
            for (id, span) in self.spans.iter().enumerate().skip(start) {
                if span.range.min() > range.max() {
                    break;
                }
                invalidated[id] = true;
            }
        }

        let mut estimates = vec![None; self.spans.len()];
        let mut additions = Vec::new();
        let mut id = 0;
        for (range, density) in disjoint_densities(ranges) {
            let mut time = range.start;
            while time < range.end {
                while self
                    .spans
                    .get(id)
                    .is_some_and(|span| i128::from(span.range.max().as_i64()) < time)
                {
                    id += 1;
                }
                let Some(span) = self.spans.get(id) else {
                    additions.extend(split_spans(time..range.end, density));
                    break;
                };
                let min = i128::from(span.range.min().as_i64());
                if time < min {
                    let end = range.end.min(min);
                    additions.extend(split_spans(time..end, density));
                    time = end;
                } else {
                    let end = range.end.min(i128::from(span.range.max().as_i64()) + 1);
                    *estimates[id].get_or_insert(0.0) += (end - time) as f64 * density;
                    time = end;
                }
            }
        }

        let mut layout_changed = !additions.is_empty();
        for (id, mut span) in std::mem::take(&mut self.spans).into_iter().enumerate() {
            let Some(estimate) = estimates[id] else {
                layout_changed = true;
                continue;
            };
            if invalidated[id] {
                span.count = (estimate.round() as usize).max(1);
                span.index = None;
            }
            additions.push(span);
        }
        additions.sort_by_key(|span| span.range.min());
        layout_changed |= coalesce(&mut additions);
        self.spans = additions;
        layout_changed
    }

    /// Total table rows, combining unresolved estimates with previously resolved counts.
    pub fn len(&self) -> usize {
        self.spans.iter().map(|span| span.count).sum()
    }

    /// Converts a global table row to a span-relative anchor for scroll preservation.
    /// Returns `None` for positions outside the current layout.
    pub fn anchor(&self, row: usize) -> Option<TextLogSpanAnchor> {
        let mut start = 0;
        for (id, span) in self.spans.iter().enumerate() {
            if row < start + span.count {
                return Some(TextLogSpanAnchor {
                    span_id: id,
                    local_row: row - start,
                });
            }
            start += span.count;
        }
        None
    }

    /// Maps an anchor from this index back to the current global row position.
    /// Refinement of earlier spans shifts the offset; shrinking the anchored span clamps
    /// the local row to its last row, or its insertion boundary when empty.
    pub fn anchor_row(&self, anchor: TextLogSpanAnchor) -> usize {
        let TextLogSpanAnchor { span_id, local_row } = anchor;
        self.spans
            .iter()
            .take(span_id)
            .map(|span| span.count)
            .sum::<usize>()
            + local_row.min(
                self.spans
                    .get(span_id)
                    .map_or(0, |span| span.count.saturating_sub(1)),
            )
    }

    /// Finds the boundary before `time`, or after equal-time rows when `inclusive` is true.
    /// Uses exact identities where retained and uniform-density estimates otherwise.
    /// Times after all spans map to the one-past-end table position.
    pub fn row_for_time(&self, time: TimeInt, inclusive: bool) -> usize {
        let mut start = 0;
        for span in &self.spans {
            if time > span.range.max() {
                start += span.count;
                continue;
            }
            if let Some(index) = &span.index {
                return start
                    + index
                        .rows
                        .partition_point(|row| row.time < time || (inclusive && row.time == time));
            }
            let min = i128::from(span.range.min().as_i64());
            let width = i128::from(span.range.max().as_i64()) - min + 1;
            let offset = (i128::from(time.as_i64()) - min + i128::from(inclusive)).clamp(0, width);
            // Both factors fit in u64; their product fits in u128 even across the full timeline.
            let row = (offset as u128 * span.count as u128 / width as u128) as usize;
            return start + row;
        }
        start
    }

    /// Whether the cursor's destination span retains exact row identities.
    /// Gaps select the next span, times after the data select the last, and an empty index
    /// is resolved. This does not imply that surrounding viewport spans are resolved.
    pub fn time_is_resolved(&self, time: TimeInt) -> bool {
        self.spans
            .iter()
            .find(|span| span.range.max() >= time)
            .or_else(|| self.spans.last())
            .is_none_or(|span| span.index.is_some())
    }

    /// Exact row identities intersecting a table viewport, with their global row offsets.
    pub fn resolved_rows(
        &self,
        visible: std::ops::Range<usize>,
    ) -> impl Iterator<Item = (usize, &[IndexedRow])> {
        let mut offset = 0;
        self.spans.iter().filter_map(move |span| {
            let start = offset;
            offset += span.count;
            let index = span.index.as_ref()?;
            let lo = visible.start.max(start);
            let hi = visible.end.min(offset);
            (lo < hi).then(|| (lo, &index.rows[lo - start..hi - start]))
        })
    }

    /// First unresolved span intersecting table rows `start..end`, as `(id, range)`.
    /// Resolved empty spans contribute no rows and are skipped. Evicted spans remain eligible
    /// for querying, even though their previously resolved counts are retained.
    pub fn next_query(&self, start: usize, end: usize) -> Option<(usize, AbsoluteTimeRange)> {
        let mut offset = 0;
        for (id, span) in self.spans.iter().enumerate() {
            let next = offset + span.count;
            if next > start && offset < end && span.index.is_none() {
                return Some((id, span.range));
            }
            offset = next;
        }
        None
    }

    /// Temporal extent of spans intersecting these rows, stable across count refinement.
    pub fn protected_range(&self, rows: std::ops::Range<usize>) -> AbsoluteTimeRange {
        let Some(first) = self.anchor(rows.start).filter(|_| !rows.is_empty()) else {
            return AbsoluteTimeRange::EMPTY;
        };
        let last = self
            .anchor(rows.end - 1)
            .map_or(self.spans.len() - 1, |anchor| anchor.span_id);
        AbsoluteTimeRange::new(
            self.spans[first.span_id].range.min(),
            self.spans[last].range.max(),
        )
    }

    /// Replaces a span's estimate with complete, time-sorted, filtered row identities.
    /// `id` must belong to this layout; protected ranges cover every active consumer's viewport.
    /// Distant identities may be evicted, but their counts and protected spans remain.
    pub fn resolve_protected(
        &mut self,
        id: usize,
        index: Arc<TextLogIndex>,
        protected: &[AbsoluteTimeRange],
    ) {
        self.spans[id].count = index.rows.len();
        self.spans[id].index = Some(if index.rows.is_empty() {
            Arc::new(TextLogIndex::default())
        } else {
            index
        });

        // Retain 32 nonempty spans near the last refinement, plus protected viewport spans.
        // This is a soft span-count target, not a bound on retained rows or bytes.
        // Exact counts survive eviction of per-instance metadata.
        const RETAIN_SPANS: usize = 32;
        let mut loaded: Vec<_> = self
            .spans
            .iter()
            .enumerate()
            .filter(|(_, span)| {
                span.index
                    .as_ref()
                    .is_some_and(|index| !index.rows.is_empty())
            })
            .map(|(other, _)| other)
            .collect();
        loaded.sort_by_key(|other| other.abs_diff(id));
        for other in loaded.into_iter().skip(RETAIN_SPANS) {
            if !protected
                .iter()
                .any(|range| range.intersects(self.spans[other].range))
            {
                self.spans[other].index = None;
            }
        }
    }
}

/// Produces disjoint half-open intervals with summed row densities.
/// Coverage is tracked separately so floating-point cancellation cannot erase sparse intervals.
fn disjoint_densities(
    ranges: impl IntoIterator<Item = (AbsoluteTimeRange, u64)>,
) -> Vec<(std::ops::Range<i128>, f64)> {
    let mut events = BTreeMap::<i128, (f64, i64)>::new();
    for (range, count) in ranges {
        if count == 0 || range.is_empty() {
            continue;
        }
        let start = i128::from(range.min().as_i64());
        let end = i128::from(range.max().as_i64()) + 1;
        let density = count as f64 / (end - start) as f64;
        let start_event = events.entry(start).or_default();
        start_event.0 += density;
        start_event.1 += 1;
        let end_event = events.entry(end).or_default();
        end_event.0 -= density;
        end_event.1 -= 1;
    }

    let mut intervals = Vec::new();
    let mut density = 0.0_f64;
    let mut previous = None;
    let mut active = 0;
    for (end, (delta, active_delta)) in events {
        if let Some(start) = previous
            && active > 0
        {
            intervals.push((start..end, density));
        }
        active += active_delta;
        density = if active == 0 {
            0.0
        } else {
            (density + delta).max(0.0)
        };
        previous = Some(end);
    }
    intervals
}

/// Merges adjacent unresolved spans while their combined count stays within [`TARGET_SPAN_ROWS`].
/// Merged spans cover any gap between them. Returns whether any spans were merged.
fn coalesce(spans: &mut Vec<TextLogNavigationSpan>) -> bool {
    let len = spans.len();
    spans.dedup_by(|span, previous| {
        let merge = previous.index.is_none()
            && span.index.is_none()
            && previous.count + span.count <= TARGET_SPAN_ROWS;
        if merge {
            previous.range = AbsoluteTimeRange::new(previous.range.min(), span.range.max());
            previous.count += span.count;
        }
        merge
    });
    spans.len() != len
}

fn split_spans(
    range: std::ops::Range<i128>,
    density: f64,
) -> impl Iterator<Item = TextLogNavigationSpan> {
    let width = range.end - range.start;
    let pieces =
        ((width as f64 * density / TARGET_SPAN_ROWS as f64).ceil() as i128).clamp(1, width);
    (0..pieces).map(move |piece| {
        let offset = |boundary: i128| {
            i128::try_from(width as u128 * boundary as u128 / pieces as u128)
                .expect("subdivision offset cannot exceed the temporal range width")
        };
        let lo = range.start + offset(piece);
        let hi = range.start + offset(piece + 1);
        TextLogNavigationSpan {
            range: AbsoluteTimeRange::new(
                i64::try_from(lo).expect("inclusive span start is a temporal timestamp"),
                i64::try_from(hi - 1).expect("inclusive span end is a temporal timestamp"),
            ),
            count: (((hi - lo) as f64 * density).round() as usize).max(1),
            index: None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        index: &mut TextLogSpanIndex,
        id: usize,
        rows: Arc<TextLogIndex>,
        protected_rows: std::ops::Range<usize>,
    ) {
        let protected = index.protected_range(protected_rows);
        index.resolve_protected(id, rows, &[protected]);
    }

    /// Component rows per fixture span; adjacent spans of this size never merge.
    const SPAN: u64 = TARGET_SPAN_ROWS as u64;

    fn range(min: i64, max: i64) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(TimeInt::new_temporal(min), TimeInt::new_temporal(max))
    }

    fn exact_rows(range: AbsoluteTimeRange) -> Arc<TextLogIndex> {
        Arc::new(TextLogIndex {
            rows: (range.min().as_i64()..=range.max().as_i64())
                .map(|time| IndexedRow {
                    time: TimeInt::new_temporal(time),
                    source: 0,
                    row_id: re_chunk_store::RowId::new(),
                    instance: 0,
                })
                .collect(),
        })
    }

    #[test]
    fn level_carry_forward_invalidates_later_spans_including_empty_results() {
        let coverage = [
            (range(0, 99), SPAN),
            (range(100, 199), SPAN),
            (range(200, 299), SPAN),
        ];
        let mut index = TextLogSpanIndex::new(coverage);
        let first = exact_rows(coverage[0].0);
        resolve(&mut index, 0, first.clone(), 0..300);
        resolve(&mut index, 1, exact_rows(coverage[1].0), 0..300);
        resolve(&mut index, 2, Arc::default(), 0..300);
        assert!(!index.reconcile(coverage, &[range(150, TimeInt::MAX.as_i64())]));
        assert!(Arc::ptr_eq(index.spans[0].index.as_ref().unwrap(), &first));
        assert!(index.spans[1].index.is_none());
        assert!(index.spans[2].index.is_none());
        let third = 100 + TARGET_SPAN_ROWS;
        assert_eq!(index.next_query(third, third + 1).unwrap().0, 2);
    }

    #[test]
    fn coverage_removal_and_gap_insertion_preserve_unaffected_identities() {
        let mut index = TextLogSpanIndex::new([(range(0, 9), SPAN), (range(20, 29), SPAN)]);
        let retained = exact_rows(range(20, 29));
        resolve(&mut index, 0, exact_rows(range(0, 9)), 0..20);
        resolve(&mut index, 1, retained.clone(), 0..20);
        assert!(index.reconcile([(range(20, 29), SPAN)], &[range(0, 9)]));
        assert_eq!(index.spans.len(), 1);
        assert_eq!(index.len(), 10);
        assert!(Arc::ptr_eq(
            index.spans[0].index.as_ref().unwrap(),
            &retained
        ));
        assert!(index.reconcile(
            [(range(10, 19), SPAN), (range(20, 29), SPAN)],
            &[range(10, 19)]
        ));
        assert_eq!(index.spans[0].range, range(10, 19));
        assert!(Arc::ptr_eq(
            index.spans[1].index.as_ref().unwrap(),
            &retained
        ));
        assert_eq!(index.next_query(0, 20).unwrap().0, 0);
    }

    #[test]
    fn representation_changes_do_not_repartition_unchanged_coverage() {
        let mut index = TextLogSpanIndex::new([(range(0, 9), SPAN), (range(10, 19), SPAN)]);
        let first = exact_rows(range(0, 9));
        let second = exact_rows(range(10, 19));
        resolve(&mut index, 0, first.clone(), 0..20);
        resolve(&mut index, 1, second.clone(), 0..20);
        assert!(!index.reconcile([(range(0, 19), 20)], &[]));
        assert_eq!(index.spans.len(), 2);
        assert!(Arc::ptr_eq(index.spans[0].index.as_ref().unwrap(), &first));
        assert!(Arc::ptr_eq(index.spans[1].index.as_ref().unwrap(), &second));
    }

    #[test]
    fn incremental_growth_matches_batched_partition() {
        let resolved = exact_rows(range(0, 0));
        let fresh = || {
            let mut index = TextLogSpanIndex::new([(range(0, 0), 1)]);
            resolve(&mut index, 0, resolved.clone(), 0..1);
            index
        };

        let mut incremental = fresh();
        for end in (10..=200).step_by(10) {
            let dirty = range(end, end);
            assert!(incremental.reconcile([(range(0, end), end as u64 / 10 + 1)], &[dirty]));
        }
        let mut batched = fresh();
        assert!(batched.reconcile([(range(0, 200), 21)], &[range(10, 200)]));

        for index in [&incremental, &batched] {
            assert_eq!(index.spans.len(), 2);
            assert!(Arc::ptr_eq(
                index.spans[0].index.as_ref().unwrap(),
                &resolved
            ));
            assert_eq!(index.spans[1].range, range(1, 200));
            assert!(index.spans[1].index.is_none());
        }
    }

    #[test]
    fn coalescing_respects_resolved_spans_and_row_budget() {
        let small = SPAN / 4;
        let chunk = |time: i64| (range(time, time + 9), small);
        let coverage = [0, 20, 40, 60, 80, 100].map(chunk);
        let mut index = TextLogSpanIndex::new(coverage);
        // Gaps between unresolved coverage do not prevent merging up to the row budget.
        assert_eq!(index.spans.len(), 2);
        assert_eq!(index.spans[0].range, range(0, 69));
        assert_eq!(index.spans[0].count, TARGET_SPAN_ROWS);
        assert_eq!(index.spans[1].range, range(80, 109));

        let rows = 0..index.len();
        resolve(&mut index, 1, exact_rows(range(80, 109)), rows);
        assert!(index.reconcile(
            std::iter::chain(coverage, [chunk(120)]),
            &[range(0, 69), range(120, 129)]
        ));
        assert_eq!(
            index.spans.len(),
            3,
            "resolved spans must separate unresolved neighbors"
        );
        assert_eq!(index.spans[2].range, range(120, 129));
    }

    #[test]
    fn temporal_boundary_singletons() {
        for time in [TimeInt::MIN, TimeInt::MAX] {
            let index = TextLogSpanIndex::new([(AbsoluteTimeRange::new(time, time), 7)]);
            assert_eq!(index.spans.len(), 1);
            assert_eq!(index.spans[0].range, AbsoluteTimeRange::new(time, time));
            assert_eq!(index.len(), 7);
            assert_eq!(index.row_for_time(time, false), 0);
            assert_eq!(index.row_for_time(time, true), 7);
        }
    }

    #[test]
    fn full_temporal_range_subdivision_and_cursor_boundaries() {
        let range = AbsoluteTimeRange::new(TimeInt::MIN, TimeInt::MAX);
        let index = TextLogSpanIndex::new([(range, 10_000)]);
        assert!(index.spans.len() > 1);
        assert_eq!(index.spans.first().unwrap().range.min(), TimeInt::MIN);
        assert_eq!(index.spans.last().unwrap().range.max(), TimeInt::MAX);
        assert_eq!(index.row_for_time(TimeInt::MIN, false), 0);
        assert_eq!(index.row_for_time(TimeInt::MAX, false), index.len() - 1);
        assert_eq!(index.row_for_time(TimeInt::MAX, true), index.len());
        for pair in index.spans.windows(2) {
            assert_eq!(pair[0].range.max().inc(), pair[1].range.min());
        }
        let mut start = 0;
        for span in &index.spans {
            assert_eq!(index.row_for_time(span.range.min(), false), start);
            start += span.count;
            assert_eq!(index.row_for_time(span.range.max(), true), start);
        }
    }

    /// Manifest component-row counts must be replaced by filtered instance counts for navigation.
    #[test]
    fn refinement_replaces_component_counts_with_instance_positions() {
        let mut index = TextLogSpanIndex::new([(range(0, 100), 2)]);
        assert_eq!(index.len(), 2);
        let first = re_chunk_store::RowId::new();
        let target = re_chunk_store::RowId::new();
        let mut rows: Vec<_> = (0..3)
            .map(|instance| IndexedRow {
                time: TimeInt::new_temporal(0),
                source: 0,
                row_id: first,
                instance,
            })
            .collect();
        rows.push(IndexedRow {
            time: TimeInt::new_temporal(100),
            source: 0,
            row_id: target,
            instance: 0,
        });

        resolve(&mut index, 0, Arc::new(TextLogIndex { rows }), 0..2);

        assert_eq!(index.len(), 4);
        assert_eq!(index.row_for_time(TimeInt::new_temporal(100), false), 3);
        assert!(index.time_is_resolved(TimeInt::new_temporal(100)));
    }

    #[test]
    fn density_sweep_preserves_gaps_and_ignores_empty_ranges() {
        assert_eq!(
            disjoint_densities([
                (range(0, 9), 10),
                (range(5, 14), 20),
                (range(15, 19), 0),
                (AbsoluteTimeRange::EMPTY, 1),
                (range(20, 29), 10),
            ]),
            [(0..5, 1.0), (5..10, 3.0), (10..15, 2.0), (20..30, 1.0)]
        );
    }

    #[test]
    fn subdivision_preserves_coverage_and_equal_timestamps() {
        let spans: Vec<_> = split_spans(-5000..5000, 1.0).collect();
        assert!(spans.len() > 1);
        assert_eq!(spans.first().unwrap().range.min().as_i64(), -5000);
        assert_eq!(spans.last().unwrap().range.max().as_i64(), 4999);
        assert_eq!(spans.iter().map(|span| span.count).sum::<usize>(), 10_000);
        for pair in spans.windows(2) {
            assert_eq!(
                pair[0].range.max().as_i64() + 1,
                pair[1].range.min().as_i64()
            );
        }

        let spans: Vec<_> = split_spans(42..43, 10_000.0).collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].range, range(42, 42));
        assert_eq!(spans[0].count, 10_000);
    }

    #[test]
    fn sparse_coverage_survives_density_cancellation() {
        let end = 86_400_000_000_000;
        let index = TextLogSpanIndex::new([(range(0, end), 2), (range(0, 0), 4096)]);
        let tail = index.spans.last().unwrap();
        assert_eq!(tail.range.max().as_i64(), end);
        assert!(tail.count > 0);
        let row = index.row_for_time(TimeInt::new_temporal(end), false);
        assert!(index.next_query(row, row + 1).is_some());
    }

    #[test]
    fn long_distance_refinement_keeps_only_bounded_span_identities() {
        let width = TARGET_SPAN_ROWS;
        #[expect(clippy::cast_possible_wrap)]
        let span_range = |span: usize| {
            let min = (span * width) as i64;
            range(min, min + width as i64 - 1)
        };
        let mut index = TextLogSpanIndex::new((0..100).map(|span| (span_range(span), SPAN)));
        for id in std::iter::chain((0..100).step_by(2), (1..100).step_by(2)) {
            let range = index.spans[id].range;
            resolve(
                &mut index,
                id,
                exact_rows(range),
                id * width..(id + 1) * width,
            );
            let retained = index
                .resolved_rows(0..index.len())
                .map(|(_, rows)| rows.len())
                .sum::<usize>();
            assert!(
                retained <= 33 * width,
                "retained {retained} instances after refining span {id}"
            );
            assert_eq!(index.len(), 100 * width);
        }
    }

    #[test]
    fn viewport_larger_than_retention_budget_converges() {
        let exact = Arc::new(TextLogIndex {
            rows: vec![IndexedRow {
                time: TimeInt::new_temporal(0),
                source: 0,
                row_id: re_chunk_store::RowId::new(),
                instance: 0,
            }],
        });
        let mut index = TextLogSpanIndex::new((0..100).map(|time| (range(time, time), SPAN)));
        for _ in 0..75 {
            let (id, _) = index.next_query(0, 75).unwrap();
            resolve(&mut index, id, exact.clone(), 0..75);
        }
        assert!(index.next_query(0, 75).is_none());
        assert_eq!(
            index
                .resolved_rows(0..75)
                .map(|(_, rows)| rows.len())
                .sum::<usize>(),
            75
        );
    }

    #[test]
    fn overlapping_chunks_have_disjoint_spans() {
        let index = TextLogSpanIndex::new([(range(0, 99), SPAN), (range(50, 149), SPAN)]);
        let span = TARGET_SPAN_ROWS;
        assert_eq!(index.len(), 2 * span);
        assert_eq!(index.spans.len(), 3);
        assert_eq!(index.spans[1].count, span);
        for pair in index.spans.windows(2) {
            assert!(pair[0].range.max() < pair[1].range.min());
        }
        assert_eq!(index.row_for_time(TimeInt::new_temporal(75), false), span);
        assert_eq!(
            index.row_for_time(TimeInt::new_temporal(75), true),
            span + span / 50
        );
    }

    #[test]
    fn ten_million_rows_are_only_metadata_until_requested() {
        let index = TextLogSpanIndex::new([(range(0, 9_999_999), 10_000_000)]);
        let max_rows = re_chunk_store::ChunkStoreConfig::DEFAULT.chunk_max_rows;
        assert!(
            index.spans.len() as u64 <= 10_000_000_u64.div_ceil(max_rows / 2),
            "navigation metadata must scale with spans, not individual rows"
        );
        assert!(index.spans.iter().all(|span| span.index.is_none()));
        let (_, query) = index.next_query(0, 100).unwrap();
        assert!(
            query.max().as_i64() < i64::try_from(max_rows).unwrap(),
            "the initial query must remain bounded by the chunk row budget: {query:?}"
        );
        assert_eq!(index.resolved_rows(0..100).count(), 0);
    }

    #[test]
    fn empty_results_advance_and_preserve_other_span_anchors() {
        let mut index = TextLogSpanIndex::new([(range(0, 9), SPAN), (range(100, 109), SPAN)]);
        let anchor = index.anchor(TARGET_SPAN_ROWS + 5).unwrap();
        resolve(&mut index, 0, Arc::new(TextLogIndex::default()), 0..20);
        assert_eq!(index.anchor_row(anchor), 5);
        assert_eq!(index.next_query(0, 10).unwrap().0, 1);
        resolve(&mut index, 1, Arc::new(TextLogIndex::default()), 0..10);
        assert_eq!(index.len(), 0);
        assert!(index.next_query(0, 10).is_none());
    }
}
