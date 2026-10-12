//! A streaming, time-ordered merge of the rows of several chunks: the inputs the planner lists
//! for it, and the merge the executor runs over them.
//TODO(RR-5857): this streaming merge feels like something datafusion could do for us if we had a
//table provider on top of a chunk provider.

use std::sync::Arc;

use arrow::array::{Array as _, ListArray};
use itertools::Itertools as _;

use re_chunk::{Chunk, ChunkId, ChunkResult, RowId};
use re_chunk_index::ChunkProvider;
use re_log_types::{TimeInt, Timeline, TimelineName};

use crate::Error;
use crate::executor::{io_batch_end, load_in_order};
use crate::plan::ChunkSlice;
use crate::view::{ChunkIdx, ChunkIndexView, TimelineSetGroup};

/// One input of a [`RowMerge`]: a slice of one chunk, with the chunk's range start on the merge
/// timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeInput {
    pub slice: ChunkSlice,

    /// This slice's earliest timestamp on the target timeline
    pub start: TimeInt,
}

impl MergeInput {
    /// The inputs of a merge of `group` on `timeline`, one of the group's timelines: a slice of
    /// each chunk `slice_of` keeps, in range-start order, as [`RowMerge::new`] requires.
    pub fn from_group(
        group: &TimelineSetGroup,
        timeline: &Timeline,
        mut slice_of: impl FnMut(ChunkIdx) -> Option<ChunkSlice>,
    ) -> Vec<Self> {
        let spans = group.per_timeline.get(timeline);
        re_log::debug_assert!(spans.is_some(), "the merge timeline is one of the group's");
        spans
            .into_iter()
            .flatten()
            .filter_map(|span| {
                Some(Self {
                    slice: slice_of(span.chunk)?,
                    start: span.time_range.min(),
                })
            })
            .collect()
    }
}

impl AsRef<ChunkSlice> for MergeInput {
    #[inline]
    fn as_ref(&self) -> &ChunkSlice {
        &self.slice
    }
}

/// Loads its inputs in IO batches and merges their rows into `(time, row_id)` order on one
/// timeline.
///
/// A pending row is final once no unloaded input can hold a row that sorts before it: its time is
/// strictly below the next unloaded input's range start (at equal times, that input may hold a row
/// with a smaller row id). Every row is final once all inputs are loaded.
///
/// Generic over caller-provided, per-row annotation.
pub struct RowMerge<T> {
    timeline: TimelineName,

    /// In range-start order on `timeline`.
    inputs: Vec<MergeInput>,

    /// Index into `inputs` of the first one not yet loaded.
    next_unloaded: usize,

    /// Sorted by `(time, row_id)`.
    pending: Vec<PendingRow<T>>,
}

/// One loaded row, not yet taken.
pub struct PendingRow<T> {
    pub time: TimeInt,
    pub row_id: RowId,

    /// What the caller attached to the row when its chunk loaded.
    pub annotation: T,

    chunk: Arc<Chunk>,
    row_idx: usize,
}

impl<T> RowMerge<T> {
    /// `inputs` must be in range-start order on `timeline`.
    pub fn new(timeline: TimelineName, inputs: Vec<MergeInput>) -> Self {
        re_log::debug_assert!(
            inputs
                .iter()
                .tuple_windows()
                .all(|(a, b)| a.start <= b.start),
            "inputs are in range-start order"
        );
        Self {
            timeline,
            inputs,
            next_unloaded: 0,
            pending: Vec::new(),
        }
    }

    /// Whether every input is loaded, so every pending row is final.
    pub fn is_exhausted(&self) -> bool {
        self.next_unloaded >= self.inputs.len()
    }

    /// Load the next IO batch of inputs.
    ///
    /// `prepare` gets each loaded chunk with its id in the chunk index, and returns the chunk to
    /// merge, possibly with columns dropped, and one annotation per row of it.
    pub async fn load_batch(
        &mut self,
        provider: &dyn ChunkProvider,
        view: &ChunkIndexView,
        mut prepare: impl FnMut(Arc<Chunk>, ChunkId) -> Result<(Arc<Chunk>, Vec<T>), Error>,
    ) -> Result<(), Error> {
        let start = self.next_unloaded;
        let end = io_batch_end(view, &self.inputs, start);
        let batch = load_in_order(provider, view, &self.inputs[start..end]).await?;
        let chunk_ids: Vec<ChunkId> = self.inputs[start..end]
            .iter()
            .map(|input| view.chunk(input.slice.chunk).chunk_id)
            .collect();
        self.next_unloaded = end;

        // One merge per batch rather than per chunk: a merge costs a pass over every pending row,
        // and with chunks that overlap in time, pending rows pile up across many chunks.
        re_tracing::profile_scope!("merge");
        let mut loaded = Vec::new();
        for (chunk, chunk_id) in std::iter::zip(batch, chunk_ids) {
            let (chunk, annotations) = prepare(chunk, chunk_id)?;
            self.extend_rows(&mut loaded, &chunk, chunk_id, annotations)?;
        }
        loaded.sort_unstable_by_key(|row| (row.time, row.row_id));

        self.pending = std::mem::take(&mut self.pending)
            .into_iter()
            .merge_by(loaded, |a, b| (a.time, a.row_id) <= (b.time, b.row_id))
            .collect();
        Ok(())
    }

    /// Append the rows of `chunk` that hold any cell to `rows`, unsorted.
    fn extend_rows(
        &self,
        rows: &mut Vec<PendingRow<T>>,
        chunk: &Arc<Chunk>,
        chunk_id: ChunkId,
        annotations: Vec<T>,
    ) -> Result<(), Error> {
        re_log::debug_assert_eq!(annotations.len(), chunk.num_rows());
        let Some(time_column) = chunk.timelines().get(&self.timeline) else {
            return Err(Error::missing_timeline(
                chunk_id,
                chunk.entity_path(),
                self.timeline,
            ));
        };
        let times = time_column.times_raw();
        let row_ids = chunk.row_ids_slice();
        let list_arrays: Vec<&ListArray> = chunk.components().list_arrays().collect();

        rows.extend(
            annotations
                .into_iter()
                .enumerate()
                .filter(|&(row, _)| !list_arrays.iter().all(|list_array| list_array.is_null(row)))
                .map(|(row, annotation)| PendingRow {
                    time: TimeInt::new_temporal(times[row]),
                    row_id: row_ids[row],
                    annotation,
                    chunk: Arc::clone(chunk),
                    row_idx: row,
                }),
        );
        Ok(())
    }

    /// The pending rows that are final, in `(time, row_id)` order.
    pub fn final_rows(&self) -> &[PendingRow<T>] {
        let len = match self.inputs.get(self.next_unloaded) {
            Some(next) => self.pending.partition_point(|row| row.time < next.start),
            None => self.pending.len(),
        };
        &self.pending[..len]
    }

    /// Remove the first `n` pending rows, which must be final, and return them as one chunk
    /// sorted by row id, with the position the first of them lands at in that chunk.
    pub fn take(&mut self, n: usize) -> ChunkResult<(Chunk, usize)> {
        re_tracing::profile_function!();

        re_log::debug_assert!(
            0 < n && n <= self.final_rows().len(),
            "only final rows can be taken"
        );
        let rows: Vec<PendingRow<T>> = self.pending.drain(..n).collect();
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_unstable_by_key(|&i| rows[i].row_id);
        let head_position = order.iter().position(|&i| i == 0).unwrap_or_default();

        let mut sources: Vec<&Arc<Chunk>> = Vec::new();
        let indices: Vec<(usize, usize)> = order
            .iter()
            .map(|&i| {
                let row = &rows[i];
                let source = sources
                    .iter()
                    .position(|source| Arc::ptr_eq(source, &row.chunk))
                    .unwrap_or_else(|| {
                        sources.push(&row.chunk);
                        sources.len() - 1
                    });
                (source, row.row_idx)
            })
            .collect();
        let sources: Vec<&Chunk> = sources.into_iter().map(|chunk| &**chunk).collect();
        Ok((Chunk::interleaved(&sources, &indices)?, head_position))
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::RowId;
    use re_chunk_index::InMemoryChunkProvider;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreId, StoreKind, Timeline};

    use super::*;
    use crate::plan::ChunkSlice;

    fn frame() -> Timeline {
        Timeline::new_sequence("frame")
    }

    /// One point per time, row ids in time order.
    fn chunk(times: &[i64]) -> Arc<Chunk> {
        let mut builder = Chunk::builder("/merge");
        for &time in times {
            builder = builder.with_component_batch(
                RowId::new(),
                [(frame(), time)],
                (MyPoints::descriptor_points(), &[MyPoint::new(1.0, 2.0)]),
            );
        }
        Arc::new(builder.build().unwrap())
    }

    fn times<T>(rows: &[PendingRow<T>]) -> Vec<i64> {
        rows.iter().map(|row| row.time.as_i64()).collect()
    }

    /// Rows of overlapping inputs come out in time order, and a chunk is released once its last
    /// row is taken.
    #[test]
    fn merge_orders_and_releases() {
        let a = chunk(&[0, 20]);
        let b = chunk(&[10, 30]);
        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let provider =
            InMemoryChunkProvider::new(&store_id, [Arc::clone(&a), Arc::clone(&b)]).unwrap();
        let view = ChunkIndexView::try_from_raw(provider.raw_manifest()).unwrap();
        let input = |chunk: &Arc<Chunk>, start: i64| MergeInput {
            slice: ChunkSlice::all(
                view.chunks()
                    .find(|(_, meta)| meta.chunk_id == chunk.id())
                    .unwrap()
                    .0,
            ),
            start: TimeInt::new_temporal(start),
        };
        let mut merge = RowMerge::new(*frame().name(), vec![input(&a, 0), input(&b, 10)]);
        let unloaded = (Arc::strong_count(&a), Arc::strong_count(&b));

        futures::executor::block_on(merge.load_batch(&provider, &view, |chunk, _| {
            let rows = chunk.num_rows();
            Ok((chunk, vec![(); rows]))
        }))
        .unwrap();
        assert!(merge.is_exhausted());
        assert_eq!(times(merge.final_rows()), [0, 10, 20, 30]);

        let (taken, head_position) = merge.take(3).unwrap();
        assert_eq!(taken.num_rows(), 3);
        assert_eq!(
            taken.row_ids_slice()[head_position],
            a.row_ids_slice()[0],
            "the head row is the one at time 0"
        );
        assert_eq!(times(&merge.pending), [30]);
        assert_eq!(merge.pending[0].row_id, b.row_ids_slice()[1]);
        assert_eq!(
            Arc::strong_count(&a),
            unloaded.0,
            "`a` has no pending row left"
        );
        assert_eq!(
            Arc::strong_count(&b),
            unloaded.1 + 1,
            "`b` still has its row at 30 pending"
        );

        merge.take(1).unwrap();
        assert_eq!(Arc::strong_count(&b), unloaded.1);
        assert!(merge.pending.is_empty());
    }
}
