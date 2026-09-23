//! Merging within a run: packing to the target, identity of what does not merge, and the gates
//! that keep incompatible chunks apart.

use std::sync::Arc;

use re_chunk::{ArrowArray as _, Chunk, ChunkId, RowId};
use re_chunk_index::ChunkProvider as _;
use re_chunk_optimizer::testing::smallest_non_splitting_target;
use re_chunk_optimizer::{analyze_chunk_index, optimize};
use re_log_types::Timeline;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_types_core::ComponentBatch as _;

use super::helpers::*;

/// `<=` fits: chunks summing exactly to the target pack together, and the output count reaches
/// the analysis lower bound.
///
/// Payload-dominated chunks, so that the per-chunk bookkeeping the accumulator collapses stays
/// negligible against the target: exactly three chunks fit, never four.
#[test]
fn exact_fill_packs() {
    let inputs: Vec<Arc<Chunk>> = (0..6)
        .map(|i| temporal_point_chunk(i + 1, "entity", &[i as i64 * 10, i as i64 * 10 + 1], 4096))
        .collect();
    let provider = provider_of(inputs.clone());

    let size = measured(&inputs[0]);
    for chunk in &inputs {
        assert_eq!(measured(chunk), size, "fixture must be uniform");
    }
    let target = 3 * size;

    let assessment = analyze_chunk_index(provider.raw_manifest(), target).unwrap();
    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs.len() as u64, assessment.merge.achievable_chunks);
    for chunk in &outputs {
        assert_eq!(chunk.num_rows(), 6);
    }
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

/// A lone chunk in its group flows through its one-input run untouched: same `Arc`, same
/// `ChunkId`.
#[test]
fn lone_chunk_identity() {
    let inputs = vec![temporal_point_chunk(1, "entity", &[0, 1], 64)];
    let provider = provider_of(inputs.clone());

    let outputs = collect(optimize(provider, &settings(1024 * 1024, 0)).unwrap());

    assert_eq!(outputs.len(), 1);
    assert!(Arc::ptr_eq(&outputs[0], &inputs[0]));
    assert_eq!(outputs[0].id(), ChunkId::from_u128(1));
}

/// A chunk whose measured size sits in the slack band `(target, 1.2 × target]` joins the run but
/// emits alone mid-run, identity preserved — and cuts its neighbors apart.
#[test]
fn band_chunk_emits_alone() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 32),
        temporal_point_chunk(2, "entity", &[10, 11, 12, 13], 512),
        temporal_point_chunk(3, "entity", &[20, 21], 32),
    ];
    let provider = provider_of(inputs.clone());

    // The smallest target whose slack band still holds the band chunk's measured size: the chunk
    // is not split, yet measures above the target.
    let target = smallest_non_splitting_target(measured(&inputs[1]));
    assert!(measured(&inputs[1]) > target);
    assert!(measured(&inputs[0]) <= target);
    assert!(measured(&inputs[2]) <= target);

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

    // Nothing fits next to the band chunk, so every buffer holds one chunk: three outputs, all
    // identity-preserved, in run order.
    assert_eq!(outputs.len(), 3);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
    }
}

#[test]
fn encoding_mismatch() {
    // Two same-entity, same-timeline chunks whose shared component uses different datatypes:
    // the index cannot see this, so the planner runs them together; the executor's
    // `concatenable` gate leaves them unmerged — two outputs, identities preserved, no error.
    let frame = Timeline::new_sequence("frame");
    let points_as_colors = Chunk::builder_with_id(ChunkId::from_u128(1), "entity")
        .with_serialized_batches(
            RowId::from_u128(1 << 32),
            [(frame, 0_i64)],
            [MyColor::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        )
        .build()
        .unwrap();
    let actual_points = Chunk::builder_with_id(ChunkId::from_u128(2), "entity")
        .with_serialized_batches(
            RowId::from_u128(2 << 32),
            [(frame, 1_i64)],
            [MyPoint::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        )
        .build()
        .unwrap();
    assert!(!points_as_colors.concatenable(&actual_points));

    let inputs = vec![Arc::new(points_as_colors), Arc::new(actual_points)];
    let provider = provider_of(inputs.clone());
    let outputs = collect(optimize(provider, &settings(1024 * 1024, 0)).unwrap());

    assert_eq!(outputs.len(), 2);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
    }
}

/// A permanently mismatched chunk in the middle of a run blocks its neighbors (adjacent-only
/// pairing) but never errors and never loses identity: the tree merge tries both pairings, gives
/// up, and emits all three in run order.
#[test]
fn mismatched_blocker_mid_run() {
    let frame = Timeline::new_sequence("frame");
    let blocker = Chunk::builder_with_id(ChunkId::from_u128(2), "entity")
        .with_serialized_batches(
            RowId::from_u128(2 << 32),
            [(frame, 10_i64)],
            [MyColor::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        )
        .build()
        .unwrap();

    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 64),
        Arc::new(blocker),
        temporal_point_chunk(3, "entity", &[20, 21], 64),
    ];
    assert!(!inputs[0].concatenable(&inputs[1]));
    assert!(!inputs[1].concatenable(&inputs[2]));

    let provider = provider_of(inputs.clone());
    let outputs = collect(optimize(provider, &settings(1024 * 1024, 0)).unwrap());

    assert_eq!(outputs.len(), 3);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
    }
}

#[test]
fn merge_correctness() {
    // Interleaved row ids across two chunks with different components: the merged chunk is
    // RowId-sorted, has a new `ChunkId`, and unions the components with null padding.
    let frame = Timeline::new_sequence("frame");
    let points = Chunk::builder_with_id(ChunkId::from_u128(1), "entity")
        .with_serialized_batches(
            RowId::from_u128(1),
            [(frame, 0_i64)],
            [MyPoint::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        )
        .with_serialized_batches(
            RowId::from_u128(3),
            [(frame, 2_i64)],
            [MyPoint::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        )
        .build()
        .unwrap();
    let colors = Chunk::builder_with_id(ChunkId::from_u128(2), "entity")
        .with_serialized_batches(
            RowId::from_u128(2),
            [(frame, 1_i64)],
            [MyColor::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_colors())
                .unwrap()],
        )
        .with_serialized_batches(
            RowId::from_u128(4),
            [(frame, 3_i64)],
            [MyColor::from_iter(0..4)
                .try_serialized(MyPoints::descriptor_colors())
                .unwrap()],
        )
        .build()
        .unwrap();

    let provider = provider_of(vec![Arc::new(points), Arc::new(colors)]);
    let outputs = collect(optimize(provider, &settings(1024 * 1024, 0)).unwrap());

    assert_eq!(outputs.len(), 1);
    let merged = &outputs[0];

    assert_ne!(merged.id(), ChunkId::from_u128(1));
    assert_ne!(merged.id(), ChunkId::from_u128(2));

    let row_ids: Vec<RowId> = merged.row_ids().collect();
    assert_eq!(row_ids, (1..=4).map(RowId::from_u128).collect::<Vec<_>>());

    // Both components are present, null-padded on the rows that lack them.
    let components = merged.components();
    let points_column = components
        .get(MyPoints::descriptor_points().component)
        .unwrap();
    let colors_column = components
        .get(MyPoints::descriptor_colors().component)
        .unwrap();
    assert_eq!(points_column.list_array.len(), 4);
    assert_eq!(colors_column.list_array.len(), 4);
    assert_eq!(points_column.list_array.null_count(), 2);
    assert_eq!(colors_column.list_array.null_count(), 2);
}
