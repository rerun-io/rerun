//! Packing of a run's outputs toward a byte target, with row guards.

use std::collections::VecDeque;
use std::sync::Arc;

use re_byte_size::SizeBytes as _;
use re_chunk::Chunk;

use crate::Error;
use crate::settings::MergeSplitSettings;

/// Packs whole chunks toward a target; splitting is the caller's job.
pub struct Accumulator {
    target: MergeSplitSettings,

    /// The output being accumulated: a stack-like sequence of merged intermediates
    /// (see [`Self::push_and_compact`]).
    entries: Vec<AccumulatorEntry>,

    /// Sum of the entries' measured bytes, maintained across pushes and merges.
    bytes: u64,

    /// Sum of the entries' rows (merges preserve it).
    rows: u64,
}

/// One merged intermediate in an accumulator.
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

impl Accumulator {
    pub fn new(target: MergeSplitSettings) -> Self {
        Self {
            target,
            entries: Vec::new(),
            bytes: 0,
            rows: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Whether any entry has an unsorted timeline, which holds the row guard to the tighter of
    /// `max_rows` and `max_rows_if_unsorted`.
    pub fn is_unsorted(&self) -> bool {
        self.entries.iter().any(|entry| !entry.sorted)
    }

    /// Add a chunk, first flushing the accumulated output to `ready` if the chunk does not fit.
    ///
    /// Returns whether an output was flushed.
    pub fn push(
        &mut self,
        chunk: Arc<Chunk>,
        ready: &mut VecDeque<Arc<Chunk>>,
    ) -> Result<bool, Error> {
        let chunk_bytes = chunk.as_ref().total_size_bytes();
        let chunk_rows = chunk.num_rows() as u64;

        let fits_bytes = self.bytes.saturating_add(chunk_bytes) <= self.target.max_bytes.get();
        let max_rows = self
            .target
            .row_guard(chunk.all_timelines_sorted() && !self.is_unsorted());
        let fits_rows =
            max_rows.is_none_or(|max| self.rows.saturating_add(chunk_rows) <= max.get());
        let does_not_fit = !fits_bytes || !fits_rows;

        // The chunk does not fit: the accumulated output is complete, and the chunk seeds the
        // next output.
        let flushed = !self.entries.is_empty() && does_not_fit;
        if flushed {
            self.flush(ready)?;
        }

        self.push_and_compact(chunk, chunk_bytes, chunk_rows)?;
        Ok(flushed)
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
        re_tracing::profile_function!();

        let sorted = chunk.all_timelines_sorted();
        self.entries.push(AccumulatorEntry {
            chunk,
            bytes,
            rows,
            sorted,
        });
        self.bytes = self.bytes.saturating_add(bytes);
        self.rows = self.rows.saturating_add(rows);

        while let [.., below, top] = self.entries.as_slice() {
            let within_2x =
                below.bytes.max(top.bytes) <= below.bytes.min(top.bytes).saturating_mul(2);
            if !within_2x {
                break;
            }

            let Some(merged) = try_merge(&below.chunk, &top.chunk, &self.target)? else {
                break;
            };

            let merged_bytes = merged.total_size_bytes();
            let merged_sorted = merged.all_timelines_sorted();
            let merged_rows = below.rows.saturating_add(top.rows);
            self.bytes = self
                .bytes
                .saturating_sub(below.bytes)
                .saturating_sub(top.bytes)
                .saturating_add(merged_bytes);
            let len = self.entries.len();
            self.entries.truncate(len - 2);
            self.entries.push(AccumulatorEntry {
                chunk: Arc::new(merged),
                bytes: merged_bytes,
                rows: merged_rows,
                sorted: merged_sorted,
            });
        }

        Ok(())
    }

    /// Flush the entries to `ready`, folded with [`merge_and_emit`].
    pub fn flush(&mut self, ready: &mut VecDeque<Arc<Chunk>>) -> Result<(), Error> {
        self.bytes = 0;
        self.rows = 0;
        let chunks = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| entry.chunk)
            .collect();
        merge_and_emit(ready, chunks, &self.target)
    }
}

/// Merge two chunks if permitted, or return `None`.
///
/// Two gates (both are invisible to the index, which is the reason we "try" merges in the first
/// place):
///
/// - `Chunk::concatenable`: the rare schema mismatch within a group (same entity and timeline
///   set, but a shared component under different datatypes).
/// - Sortedness: the result must be within the row guard for its own sortedness. Merging two
///   individually sorted chunks can come out unsorted, and ranges cannot predict it when the row
///   ids interleave — so the merge is tried and judged on the real result. This is what keeps
///   every emitted chunk within the unsorted guard.
//TODO(RR-5527): additional data in the index might allow predicting mergeabilty, so we don't have
//to "try".
fn try_merge(
    left: &Chunk,
    right: &Chunk,
    target: &MergeSplitSettings,
) -> Result<Option<Chunk>, Error> {
    if !left.concatenable(right) {
        return Ok(None);
    }

    let merged = Chunk::concat_and_sort(left, right)
        .map_err(|err| Error::merge_chunks(left.entity_path(), err))?;

    let acceptable = target
        .row_guard(merged.all_timelines_sorted())
        .is_none_or(|max| merged.num_rows() as u64 <= max.get());

    Ok(acceptable.then_some(merged))
}

/// Fold the accumulator's entries and emit the results in entry order.
///
/// [`Accumulator::push_and_compact`] deliberately leaves entries whose sizes are more than 2×
/// apart, so this merges everything [`try_merge`] allows.
fn merge_and_emit(
    ready: &mut VecDeque<Arc<Chunk>>,
    mut chunks: Vec<Arc<Chunk>>,
    target: &MergeSplitSettings,
) -> Result<(), Error> {
    re_tracing::profile_function!();

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
            if let Some(merged) = try_merge(&left, &right, target)? {
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
