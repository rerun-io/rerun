//! Assertion helpers for `FindChunks` candidate batches. They only inspect the fixed response
//! schema, so they apply to every server.

use std::collections::BTreeSet;

use arrow::array::RecordBatch;
use re_protos::cloud::v1alpha1::ext::ChunkCandidatesDataframe;
use re_types_core::ChunkId;

/// The `chunk_id` values of `batches`, deduplicated and ordered.
pub fn chunk_ids(batches: &[RecordBatch]) -> BTreeSet<ChunkId> {
    let mut ids = BTreeSet::new();
    for batch in batches {
        let column = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column in FindChunks response");
        ids.extend(column.iter_owned());
    }
    ids
}

/// Asserts that `batches` is exactly one batch with no rows and the fixed candidate schema.
/// `FindChunks` answers every empty selection this way, so the client still receives the
/// response schema.
pub fn assert_one_empty_batch(batches: &[RecordBatch], context: &str) {
    assert_eq!(
        batches.len(),
        1,
        "{context}: an empty selection must return exactly one batch"
    );
    assert_eq!(
        batches[0].num_rows(),
        0,
        "{context}: an empty selection's one batch must have zero rows"
    );
    assert_eq!(
        batches[0].schema(),
        ChunkCandidatesDataframe::empty_record_batch().schema(),
        "{context}: an empty selection must keep the fixed candidate schema"
    );
}
