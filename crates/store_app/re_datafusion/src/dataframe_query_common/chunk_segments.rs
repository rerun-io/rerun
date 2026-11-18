use ahash::HashMap;
use arrow::array::{BooleanArray, RecordBatch};
use arrow::error::ArrowError;

use re_dataframe::external::re_chunk::{Chunk, ChunkId};
use re_protos::cloud::v1alpha1::ext::ChunkCandidatesDataframe;
use re_protos::common::v1alpha1::ext::SegmentId;

/// Chunks tagged with the segment they were requested for.
pub type ChunksWithSegment = Vec<(Chunk, SegmentId)>;

/// The segments that each chunk of a request batch was requested for.
///
/// A fetched chunk carries the recording id of the store it was logged to, which is not always
/// the segment it is registered as. The segment comes from the request rows instead.
///
/// One chunk id may be requested for several segments. It is then the same chunk: it is fetched
/// once, and attributed to each of them.
#[derive(Default)]
pub struct ChunkSegments {
    segments_of_chunk: HashMap<ChunkId, Vec<SegmentId>>,
}

impl ChunkSegments {
    /// Records the segments of each chunk of `batch`, one batch of a request.
    ///
    /// Returns the rows of `batch` to fetch: the first row of each chunk id that no earlier
    /// batch of the request asked for. This may be an empty batch.
    pub fn add_batch(&mut self, batch: &RecordBatch) -> Result<RecordBatch, ArrowError> {
        re_tracing::profile_function!();

        let chunk_ids = ChunkCandidatesDataframe::COLUMN_CHUNK_ID.extract(batch)?;
        let segment_ids = ChunkCandidatesDataframe::COLUMN_CHUNK_SEGMENT_ID.extract(batch)?;

        let mut is_first_row_of_chunk = Vec::with_capacity(batch.num_rows());
        for (chunk_id, segment_id) in
            std::iter::zip(chunk_ids.into_iter_owned(), segment_ids.into_iter_owned())
        {
            let segments = self.segments_of_chunk.entry(chunk_id).or_default();
            is_first_row_of_chunk.push(segments.is_empty());
            if !segments.contains(&segment_id) {
                segments.push(segment_id);
            }
        }

        if is_first_row_of_chunk.iter().all(|is_first| *is_first) {
            Ok(batch.clone())
        } else {
            arrow::compute::filter_record_batch(batch, &BooleanArray::from(is_first_row_of_chunk))
        }
    }

    /// Attributes fetched chunks to every segment they were requested for.
    ///
    /// A chunk that no row of the request asked for belongs to no segment, and is dropped.
    pub fn attribute(&self, chunks: impl IntoIterator<Item = Chunk>) -> ChunksWithSegment {
        let mut attributed = ChunksWithSegment::new();
        for chunk in chunks {
            let Some((last_segment, other_segments)) = self
                .segments_of_chunk
                .get(&chunk.id())
                .and_then(|segments| segments.split_last())
            else {
                re_log::error_once!("fetched a chunk that was not requested; dropping");
                continue;
            };
            for segment_id in other_segments {
                attributed.push((chunk.clone(), segment_id.clone()));
            }
            attributed.push((chunk, last_segment.clone()));
        }
        attributed
    }

    /// Attributes chunks that were fetched in any order, each tagged with the index of its row
    /// in the rows to fetch returned by [`Self::add_batch`].
    ///
    /// The result follows the order of those rows.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn attribute_in_row_order(&self, mut chunks: Vec<(usize, Chunk)>) -> ChunksWithSegment {
        chunks.sort_by_key(|(row_idx, _chunk)| *row_idx);
        self.attribute(chunks.into_iter().map(|(_row_idx, chunk)| chunk))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::RecordBatchOptions;
    use arrow::datatypes::Schema;
    use re_log_types::EntityPath;

    use super::*;

    /// A request batch with one row per `(chunk id, segment id)` pair.
    fn request_batch(rows: &[(ChunkId, &str)]) -> RecordBatch {
        let (chunk_id_field, chunk_ids) = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .new_from_values(rows.iter().map(|(chunk_id, _)| *chunk_id))
            .into_dyn()
            .into_parts();
        let (segment_id_field, segment_ids) = ChunkCandidatesDataframe::COLUMN_CHUNK_SEGMENT_ID
            .new_from_values(
                rows.iter()
                    .map(|(_, segment_id)| SegmentId::from(*segment_id)),
            )
            .into_dyn()
            .into_parts();

        RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(
                vec![chunk_id_field, segment_id_field],
                Default::default(),
            )),
            vec![chunk_ids, segment_ids],
            &RecordBatchOptions::new().with_row_count(Some(rows.len())),
        )
        .unwrap()
    }

    fn chunk(chunk_id: ChunkId) -> Chunk {
        Chunk::builder_with_id(chunk_id, EntityPath::root())
            .build()
            .unwrap()
    }

    fn ids(attributed: &ChunksWithSegment) -> Vec<(ChunkId, &str)> {
        attributed
            .iter()
            .map(|(chunk, segment_id)| (chunk.id(), segment_id.as_str()))
            .collect()
    }

    /// Chunks come back in any order: each goes to the segment of its request row.
    #[test]
    fn a_chunk_goes_to_the_segment_of_its_request_row() {
        let (a, b) = (ChunkId::new(), ChunkId::new());
        let batch = request_batch(&[(a, "seg-a"), (b, "seg-b")]);

        let mut chunk_segments = ChunkSegments::default();
        let rows_to_fetch = chunk_segments.add_batch(&batch).unwrap();
        let attributed = chunk_segments.attribute([chunk(b), chunk(a)]);

        assert_eq!(rows_to_fetch, batch);
        assert_eq!(ids(&attributed), [(b, "seg-b"), (a, "seg-a")]);
    }

    /// One chunk id under two segments is the same chunk: it is fetched once and goes to both.
    #[test]
    fn a_chunk_requested_for_two_segments_is_fetched_once_and_goes_to_both() {
        let (shared, other) = (ChunkId::new(), ChunkId::new());
        let batch = request_batch(&[(shared, "seg-a"), (shared, "seg-b"), (other, "seg-b")]);

        let mut chunk_segments = ChunkSegments::default();
        let rows_to_fetch = chunk_segments.add_batch(&batch).unwrap();
        let attributed = chunk_segments.attribute([chunk(shared), chunk(other)]);

        assert_eq!(
            rows_to_fetch,
            request_batch(&[(shared, "seg-a"), (other, "seg-b")])
        );
        assert_eq!(
            ids(&attributed),
            [(shared, "seg-a"), (shared, "seg-b"), (other, "seg-b")]
        );
    }

    /// A chunk id repeated across the batches of one request is fetched once.
    #[test]
    fn a_chunk_repeated_across_batches_is_fetched_once() {
        let (shared, other) = (ChunkId::new(), ChunkId::new());
        let batches = [
            request_batch(&[(shared, "seg-a")]),
            request_batch(&[(shared, "seg-a"), (other, "seg-a")]),
        ];

        let mut chunk_segments = ChunkSegments::default();
        let rows_to_fetch = batches
            .each_ref()
            .map(|batch| chunk_segments.add_batch(batch).unwrap());
        let attributed = chunk_segments.attribute([chunk(other), chunk(shared)]);

        assert_eq!(
            rows_to_fetch,
            [
                request_batch(&[(shared, "seg-a")]),
                request_batch(&[(other, "seg-a")])
            ]
        );
        assert_eq!(ids(&attributed), [(other, "seg-a"), (shared, "seg-a")]);
    }

    /// Row indices are those of the rows to fetch, where a shared chunk has a single row.
    #[test]
    fn chunks_fetched_out_of_order_come_back_in_row_order() {
        let (shared, other) = (ChunkId::new(), ChunkId::new());
        let batch = request_batch(&[(shared, "seg-a"), (shared, "seg-b"), (other, "seg-b")]);

        let mut chunk_segments = ChunkSegments::default();
        let rows_to_fetch = chunk_segments.add_batch(&batch).unwrap();
        let attributed =
            chunk_segments.attribute_in_row_order(vec![(1, chunk(other)), (0, chunk(shared))]);

        assert_eq!(rows_to_fetch.num_rows(), 2);
        assert_eq!(
            ids(&attributed),
            [(shared, "seg-a"), (shared, "seg-b"), (other, "seg-b")]
        );
    }

    /// A chunk that no request row asked for belongs to no segment.
    #[test]
    fn an_unrequested_chunk_is_dropped() {
        let batch = request_batch(&[(ChunkId::new(), "seg-a")]);

        let mut chunk_segments = ChunkSegments::default();
        chunk_segments.add_batch(&batch).unwrap();
        let attributed = chunk_segments.attribute([chunk(ChunkId::new())]);

        assert_eq!(ids(&attributed), []);
    }
}
