//! Splitting oversized chunks: where the cuts fall and how the pieces pack with their neighbors.

use std::sync::Arc;

use re_chunk_optimizer::optimize;
use re_chunk_optimizer::testing::should_split_chunk;

use super::helpers::*;

/// An oversized chunk mid-run cuts the accumulator, splits into near-target pieces, and never merges
/// with its neighbors — identity preserved for the neighbors and for nothing else.
#[test]
fn oversized_chunk_splits_mid_run() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 32),
        temporal_point_chunk(2, "entity", &(10..26).collect::<Vec<_>>(), 512),
        temporal_point_chunk(3, "entity", &[30, 31], 32),
    ];
    let provider = provider_of(inputs.clone());

    // A target that holds both small chunks together, with the big chunk far past the band.
    let target = 3 * measured(&inputs[0]);
    assert!(should_split_chunk(measured(&inputs[1]), target));

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

    // First small chunk (cut by the split), then the pieces, then the tail small chunk.
    assert!(outputs.len() > 3);
    assert!(Arc::ptr_eq(&outputs[0], &inputs[0]));
    assert!(Arc::ptr_eq(outputs.last().unwrap(), &inputs[2]));
    let pieces = &outputs[1..outputs.len() - 1];
    assert_eq!(pieces.iter().map(|c| c.num_rows()).sum::<usize>(), 16);
    for piece in pieces {
        assert_ne!(piece.id(), inputs[1].id());
    }
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

/// Split pieces re-enter the run: a split's under-target tail piece merges with the following
/// chunk instead of stranding a runt.
#[test]
fn split_tail_coalesces() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &(0..7).collect::<Vec<_>>(), 1024),
        temporal_point_chunk(2, "entity", &[10], 1024),
    ];
    let provider = provider_of(inputs.clone());

    // Three rows' worth: the big chunk splits [3, 3, 1]; the small chunk fits under the target.
    let target = 3 * (measured(&inputs[0]) / 7);
    assert!(
        should_split_chunk(measured(&inputs[0]), target),
        "must trip the split"
    );
    assert!(measured(&inputs[1]) <= target);

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

    // The one-row tail piece coalesces with the following one-row chunk.
    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![3, 3, 2]
    );
    assert_eq!(row_set(&inputs), row_set(&outputs));
}
