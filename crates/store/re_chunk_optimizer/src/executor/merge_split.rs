//! Execution of one [`PlanUnit::MergeSplitRun`](crate::plan::PlanUnit): cut and split outputs on
//! measured sizes.

use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::ops::ControlFlow;
use std::sync::Arc;

use re_byte_size::SizeBytes as _;
use re_chunk::Chunk;
use re_chunk_index::ChunkProvider;

use super::accumulator::Accumulator;
use super::cut::{Budget, cut_to_fit};
use super::{io_batch_end, load_in_order};
use crate::Error;
use crate::plan::ChunkSlice;
use crate::settings::MergeSplitSettings;
use crate::view::ChunkIndexView;

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

/// The largest measured size the slack band of `max_bytes` holds: [`should_split_chunk`] is false
/// at this size and true just above it.
fn largest_non_splitting_size(max_bytes: u64) -> u64 {
    #[expect(clippy::cast_possible_truncation)] // at most 1.2 × a `u64`, from a real chunk size
    let size =
        (u128::from(max_bytes) * SPLIT_THRESHOLD_FACTOR_NUM / SPLIT_THRESHOLD_FACTOR_DEN) as u64;
    size
}

/// Executor state of a single [`PlanUnit::MergeSplitRun`](crate::plan::PlanUnit).
pub struct MergeSplitRunState {
    slices: Vec<ChunkSlice>,
    target: MergeSplitSettings,

    /// Index into `slices` of the first one not yet fetched.
    next_unfetched_slice: usize,

    /// Loaded input chunks not yet processed.
    ///
    /// Refilled only when empty, by one `load_chunks` batch, so it never holds more than one
    /// decoded batch. An oversized chunk's pieces are pushed to its front and replace it.
    pending: VecDeque<Arc<Chunk>>,

    accumulator: Accumulator,
}

impl MergeSplitRunState {
    pub fn new(slices: Vec<ChunkSlice>, target: MergeSplitSettings) -> Self {
        Self {
            slices,
            target,
            next_unfetched_slice: 0,
            pending: VecDeque::new(),
            accumulator: Accumulator::new(target),
        }
    }

    /// Drive the run until it emits at least one output into `ready` or finishes.
    ///
    /// Returns [`ControlFlow::Break`] when the run is done: its slices are exhausted and its
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

                if needs_split(&chunk, chunk_bytes, &self.target) {
                    // Replace the oversized chunk by its pieces in the input stream.
                    if let Some(pieces) = cut_to_fit(
                        &chunk,
                        chunk_bytes,
                        self.room_budget(&chunk),
                        &self.full_budget(&chunk),
                    ) {
                        for piece in pieces.into_iter().rev() {
                            self.pending.push_front(piece);
                        }
                        continue;
                    }

                    // The cut made no progress: a single row, or one that exceeds the target on
                    // its own. Re-queueing would loop forever, so admit the chunk as-is — it
                    // emits alone, identity preserved, like a band chunk.
                }

                if self.accumulator.push(chunk, ready)? {
                    return Ok(ControlFlow::Continue(()));
                }
            } else if self.next_unfetched_slice < self.slices.len() {
                let end = io_batch_end(view, &self.slices, self.next_unfetched_slice);
                let batch =
                    load_in_order(provider, view, &self.slices[self.next_unfetched_slice..end])
                        .await?;
                self.next_unfetched_slice = end;
                self.pending = batch.into();
            } else {
                self.accumulator.flush(ready)?;
                return Ok(ControlFlow::Break(()));
            }
        }
    }

    /// The budget of an oversized chunk's first piece: what is left under the target in bytes and
    /// rows, so the piece joins the accumulator. `None` when the accumulator is empty, since the
    /// piece then seeds an output rather than joining one.
    fn room_budget(&self, chunk: &Chunk) -> Option<Budget> {
        if self.accumulator.is_empty() {
            return None;
        }
        let row_guard = self
            .target
            .row_guard(chunk.all_timelines_sorted() && !self.accumulator.is_unsorted());
        let room = self
            .target
            .max_bytes
            .get()
            .saturating_sub(self.accumulator.bytes());
        Some(Budget {
            max_row_bytes: room,
            max_rows: row_guard
                .map_or(u64::MAX, NonZeroU64::get)
                .saturating_sub(self.accumulator.rows()),
            max_measured_bytes: room,
        })
    }

    /// The budget of a piece that seeds an output of its own: a whole target, confirmed within the
    /// slack band so it is not split again on admission.
    fn full_budget(&self, chunk: &Chunk) -> Budget {
        let max_bytes = self.target.max_bytes.get();
        Budget {
            max_row_bytes: max_bytes,
            max_rows: self
                .target
                .row_guard(chunk.all_timelines_sorted())
                .map_or(u64::MAX, NonZeroU64::get),
            max_measured_bytes: largest_non_splitting_size(max_bytes),
        }
    }
}

/// Whether an incoming chunk must be split instead of joining the accumulator.
fn needs_split(chunk: &Chunk, measured_bytes: u64, target: &MergeSplitSettings) -> bool {
    if chunk.num_rows() <= 1 {
        return false;
    }

    let rows = chunk.num_rows() as u64;
    let over_bytes = should_split_chunk(measured_bytes, target.max_bytes.get());
    let over_rows = target
        .row_guard(chunk.all_timelines_sorted())
        .is_some_and(|max| rows > max.get());

    over_bytes || over_rows
}
