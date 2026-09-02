//! Execution of one [`PlanUnit::MergeSplitRun`](crate::plan::PlanUnit): cut and split outputs on
//! measured sizes.

use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::ops::ControlFlow;
use std::sync::Arc;

use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, SplitRowsOptions};
use re_log_encoding::ChunkProvider;

use crate::Error;
use crate::settings::MergeSplitSettings;
use crate::view::{ChunkIdx, ChunkIndexView};

use super::load_in_order;

/// Minimum on-disk bytes (`rrd_byte_size`) requested per `load_chunks` call within a merge run,
/// to give the provider a chance to coalesce reads. (Note: this is an IO floor, not a memory
/// budget.)
const MIN_LOAD_CHUNK_BATCH: u64 = 8 * 1024 * 1024;

/// Only split chunks if their size is above the `target_size * SPLIT_THRESHOLD_FACTOR`.
///
/// The product of chunk merging can often overshoot the target due to per-chunk framing and
/// padding. In order to achieve idempotency across optimization run — specifically avoid merge-
/// split back-and-forth behavior — we introduce hysteresis via this factor.
///
/// Expressed as fraction to avoid float arithmetics.
const SPLIT_THRESHOLD_FACTOR_NUM: u128 = 6;
const SPLIT_THRESHOLD_FACTOR_DEN: u128 = 5;

pub fn should_split_chunk(size: u64, max_bytes: u64) -> bool {
    SPLIT_THRESHOLD_FACTOR_DEN * u128::from(size)
        > SPLIT_THRESHOLD_FACTOR_NUM * u128::from(max_bytes)
}

/// The smallest byte target whose slack band still holds a chunk of the given measured size:
/// [`should_split_chunk`] is false at this target and true just below it.
///
/// Exposed for testing.
pub fn smallest_non_splitting_target(size: u64) -> u64 {
    #[expect(clippy::cast_possible_truncation)] // the result is at most `size`
    let target =
        (u128::from(size) * SPLIT_THRESHOLD_FACTOR_DEN).div_ceil(SPLIT_THRESHOLD_FACTOR_NUM) as u64;
    target
}

/// Executor state of a single [`PlanUnit::MergeSplitRun`](crate::plan::PlanUnit).
pub struct MergeSplitRunState {
    inputs: Vec<ChunkIdx>,
    target: MergeSplitSettings,

    /// Index into `inputs` of the first chunk not yet fetched.
    next_unfetched_input: usize,

    /// Already loaded but not yet processed input chunks.
    ///
    /// Refilled only when empty, by one `load_chunks` batch; a split chunk's pieces are pushed
    /// back to its front. So it holds at most one decoded batch plus one split chunk's pieces.
    pending: VecDeque<Arc<Chunk>>,

    /// The output being accumulated: a stack-like sequence of merged intermediates
    /// (see [`Self::push_and_compact`]).
    accumulator: Vec<AccumulatorEntry>,

    /// Sum of the accumulator entries' measured bytes, maintained across pushes and merges.
    accumulator_bytes: u64,

    /// Sum of the accumulator entries' rows (merges preserve it).
    accumulator_rows: u64,
}

/// One merged intermediate in a run's accumulator.
struct AccumulatorEntry {
    chunk: Arc<Chunk>,

    /// The entry's measured `total_size_bytes`.
    ///
    /// This is re-measured after every merge, so effect of per-chunk framing and padding is
    /// measured instead of estimated.
    bytes: u64,

    /// The entry's row count.
    rows: u64,

    /// Whether the entry's timelines are all sorted.
    sorted: bool,
}

impl MergeSplitRunState {
    pub fn new(inputs: Vec<ChunkIdx>, target: MergeSplitSettings) -> Self {
        Self {
            inputs,
            target,
            next_unfetched_input: 0,
            pending: VecDeque::new(),
            accumulator: Vec::new(),
            accumulator_bytes: 0,
            accumulator_rows: 0,
        }
    }

    /// Drive the run until it emits at least one output into `ready` or finishes.
    ///
    /// Returns [`ControlFlow::Break`] when the run is done: its inputs are exhausted and its
    /// final output emitted. [`ControlFlow::Continue`] means there is more to step through.
    pub async fn step(
        &mut self,
        provider: &dyn ChunkProvider,
        view: &ChunkIndexView,
        ready: &mut VecDeque<Arc<Chunk>>,
    ) -> Result<ControlFlow<()>, Error> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                let chunk_bytes = chunk.as_ref().total_size_bytes();
                let chunk_rows = chunk.num_rows() as u64;

                if needs_split(&chunk, chunk_bytes, &self.target) {
                    // Replace the oversized chunk by its pieces in the input stream.
                    // `SplitRowsOptions` follows the legacy convention: `0` disables a limit.
                    let options = SplitRowsOptions {
                        chunk_max_bytes: self.target.max_bytes.get(),
                        chunk_max_rows: self.target.max_rows.map_or(0, NonZeroU64::get),
                        chunk_max_rows_if_unsorted: self
                            .target
                            .max_rows_if_unsorted
                            .map_or(0, NonZeroU64::get),
                    };
                    let pieces = Chunk::split_rows(Arc::clone(&chunk), &options);
                    if pieces.len() > 1 {
                        for piece in pieces.into_iter().rev() {
                            self.pending.push_front(piece);
                        }
                        continue;
                    }

                    // The split made no progress: `split_rows` estimates rows per piece from
                    // floored average bytes-per-row, which can round `target_rows` up to the whole
                    // chunk. Re-queueing would loop forever, so discard the copy and admit the
                    // original as-is — it emits alone, identity preserved, like a band chunk.
                }

                let fits_bytes = self.accumulator_bytes.saturating_add(chunk_bytes)
                    <= self.target.max_bytes.get();
                let max_rows = if chunk.all_timelines_sorted() && !self.is_accumulator_unsorted() {
                    self.target.max_rows
                } else {
                    self.target.max_rows_if_unsorted
                };
                let fits_rows = max_rows.is_none_or(|max| {
                    self.accumulator_rows.saturating_add(chunk_rows) <= max.get()
                });
                let does_not_fit = !fits_bytes || !fits_rows;

                // The chunk does not fit: the accumulated output is complete, and the chunk
                // seeds the next output.
                if !self.accumulator.is_empty() && does_not_fit {
                    self.flush_accumulator(ready)?;
                    self.push_and_compact(chunk, chunk_bytes, chunk_rows)?;
                    return Ok(ControlFlow::Continue(()));
                }

                // The chunk joins the output.
                self.push_and_compact(chunk, chunk_bytes, chunk_rows)?;
            } else if self.next_unfetched_input < self.inputs.len() {
                // Fetch the next IO batch: the shortest prefix of the remaining inputs whose
                // on-disk size reaches the minimum. The loop always takes at least one chunk, so
                // progress is guaranteed.
                let mut end = self.next_unfetched_input;
                let mut batch_bytes = 0_u64;
                while end < self.inputs.len() && batch_bytes < MIN_LOAD_CHUNK_BATCH {
                    batch_bytes =
                        batch_bytes.saturating_add(view.chunk(self.inputs[end]).rrd_byte_size);
                    end += 1;
                }

                let batch =
                    load_in_order(provider, view, &self.inputs[self.next_unfetched_input..end])
                        .await?;
                self.next_unfetched_input = end;
                self.pending = batch.into();
            } else {
                self.flush_accumulator(ready)?;
                return Ok(ControlFlow::Break(()));
            }
        }
    }

    /// Whether any accumulator entry has an unsorted timeline, which switches the row guard to
    /// `max_rows_if_unsorted`.
    fn is_accumulator_unsorted(&self) -> bool {
        self.accumulator.iter().any(|entry| !entry.sorted)
    }

    /// Push one decoded chunk onto the accumulator and compact it.
    ///
    /// Compaction merges for the sake of the *measurement*: the cut decision needs honest sizes,
    /// and the only way to know what a merge measures is to do it. Since a merge copies both
    /// inputs into a fresh chunk, merging on every push would copy each row O(n) times; instead,
    /// the accumulator merges its top two entries only while they are within 2× of each other by
    /// measurement (LSM/doubling style), which keeps it to O(log n) copies per row.
    ///
    /// The lopsided pairs this leaves behind are [`merge_and_emit`]'s job, once the output is
    /// final.
    // TODO(ab): so far, we stayed close to how the legacy optimization operates: no rewrite of
    // row ids, meaning no possibility to force-sort unsorted chunks/merge candidate. Having the
    // ability to sort chunk here would be beneficial.
    // TODO(RR-5587): it would probably be easy to implement a N-ary `concat_and_sort` operator
    // (Arrow has a N-ary `concat`). This could further reduce the number of copies and simplify
    // things. That said, we already perform better than legacy on maximally fragmented inputs, so
    // this is likely not the bottleneck.
    fn push_and_compact(&mut self, chunk: Arc<Chunk>, bytes: u64, rows: u64) -> Result<(), Error> {
        let sorted = chunk.all_timelines_sorted();
        self.accumulator.push(AccumulatorEntry {
            chunk,
            bytes,
            rows,
            sorted,
        });
        self.accumulator_bytes = self.accumulator_bytes.saturating_add(bytes);
        self.accumulator_rows = self.accumulator_rows.saturating_add(rows);

        while let [.., below, top] = self.accumulator.as_slice() {
            let within_2x =
                below.bytes.max(top.bytes) <= below.bytes.min(top.bytes).saturating_mul(2);
            if !within_2x {
                break;
            }

            let Some(merged) =
                try_merge(&below.chunk, &top.chunk, self.target.max_rows_if_unsorted)?
            else {
                break;
            };

            let merged_bytes = merged.total_size_bytes();
            let merged_sorted = merged.all_timelines_sorted();
            let merged_rows = below.rows.saturating_add(top.rows);
            let Some(top) = self.accumulator.pop() else {
                break;
            };
            let Some(below) = self.accumulator.pop() else {
                break;
            };
            self.accumulator_bytes = self
                .accumulator_bytes
                .saturating_sub(below.bytes)
                .saturating_sub(top.bytes)
                .saturating_add(merged_bytes);
            self.accumulator.push(AccumulatorEntry {
                chunk: Arc::new(merged),
                bytes: merged_bytes,
                rows: merged_rows,
                sorted: merged_sorted,
            });
        }

        Ok(())
    }

    /// Fold the accumulator entries with [`merge_and_emit`] and push them to the output.
    fn flush_accumulator(&mut self, ready: &mut VecDeque<Arc<Chunk>>) -> Result<(), Error> {
        self.accumulator_bytes = 0;
        self.accumulator_rows = 0;
        let chunks = std::mem::take(&mut self.accumulator)
            .into_iter()
            .map(|entry| entry.chunk)
            .collect();
        merge_and_emit(ready, chunks, self.target.max_rows_if_unsorted)
    }
}

/// Merge two chunks if permitted, or return `None`.
///
/// Two gates (both are invisible to the index, which is the reason we "try" merges in the first
/// place):
///
/// - `Chunk::concatenable`: the rare schema mismatch within a group (same entity and timeline
///   set, but a shared component under different datatypes).
/// - Sortedness: the result must be time-sorted, or within `max_rows_if_unsorted` (if any).
///   Merging two individually sorted chunks can come out unsorted, and ranges cannot predict it
///   when the row ids interleave — so the merge is tried and judged on the real result. This is
///   what keeps every emitted chunk within the unsorted guard.
//TODO(RR-5527): additional data in the index might allow predicting mergeabilty, so we don't have
//to "try".
fn try_merge(
    left: &Chunk,
    right: &Chunk,
    max_rows_if_unsorted: Option<NonZeroU64>,
) -> Result<Option<Chunk>, Error> {
    if !left.concatenable(right) {
        return Ok(None);
    }

    let merged = Chunk::concat_and_sort(left, right)
        .map_err(|err| Error::merge_chunks(left.entity_path(), err))?;

    let acceptable = merged.all_timelines_sorted()
        || max_rows_if_unsorted.is_none_or(|max| merged.num_rows() as u64 <= max.get());

    Ok(acceptable.then_some(merged))
}

/// Fold the accumulator's entries and emit the results in entry order.
///
/// [`MergeSplitRunState::push_and_compact`] deliberately leaves entries whose sizes are more
/// than 2× apart, so this merges everything [`try_merge`] allows.
fn merge_and_emit(
    ready: &mut VecDeque<Arc<Chunk>>,
    mut chunks: Vec<Arc<Chunk>>,
    max_rows_if_unsorted: Option<NonZeroU64>,
) -> Result<(), Error> {
    let mut skip_first = false;
    let mut rounds_without_merge = 0;

    // repeatedly attempt to merge neighboring chunks
    while chunks.len() > 1 && rounds_without_merge < 2 {
        let mut next_round: Vec<Arc<Chunk>> = Vec::with_capacity(chunks.len() / 2 + 1);
        let mut merged_any = false;

        let mut iter = chunks.into_iter();

        // Alternate rounds carry the first chunk over unpaired, so every chunk gets to meet both
        // neighbors instead of retrying the same pairing.
        if skip_first && let Some(first) = iter.next() {
            next_round.push(first);
        }
        skip_first = !skip_first;

        while let Some(left) = iter.next() {
            let Some(right) = iter.next() else {
                next_round.push(left);
                break;
            };
            if let Some(merged) = try_merge(&left, &right, max_rows_if_unsorted)? {
                next_round.push(Arc::new(merged));
                merged_any = true;
            } else {
                next_round.push(left);
                next_round.push(right);
            }
        }

        chunks = next_round;
        rounds_without_merge = if merged_any {
            0
        } else {
            rounds_without_merge + 1
        };
    }

    ready.extend(chunks);
    Ok(())
}

/// Whether an incoming chunk must be split instead of joining the accumulator.
fn needs_split(chunk: &Chunk, measured_bytes: u64, target: &MergeSplitSettings) -> bool {
    if chunk.num_rows() <= 1 {
        return false;
    }

    let rows = chunk.num_rows() as u64;
    let over_bytes = should_split_chunk(measured_bytes, target.max_bytes.get());
    let over_rows = target.max_rows.is_some_and(|max| rows > max.get());
    let over_rows_unsorted = target
        .max_rows_if_unsorted
        .is_some_and(|max| rows > max.get())
        && !chunk.all_timelines_sorted();

    over_bytes || over_rows || over_rows_unsorted
}
