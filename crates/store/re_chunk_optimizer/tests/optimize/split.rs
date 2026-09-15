//! Splitting oversized chunks: where the cuts fall and how the pieces pack with their neighbors.

use std::sync::Arc;

use re_chunk_optimizer::optimize;
use re_chunk_optimizer::testing::should_split_chunk;

use super::helpers::*;

/// An oversized chunk whose every row is wider than the room next to its neighbors cuts the
/// accumulator: the neighbors keep identity, and the chunk splits into single-row pieces that each
/// emit alone.
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

/// An oversized chunk is cut to pack: its first piece fills what the accumulator has left, the
/// next pieces fill whole targets, and only the run's last output is under-filled.
#[test]
fn split_fills_the_accumulator() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 1024),
        temporal_point_chunk(2, "entity", &(10..26).collect::<Vec<_>>(), 1024),
    ];
    let provider = provider_of(inputs.clone());

    // Five rows' worth: the small chunk fills two of them, so the big chunk's first piece is
    // three rows, not five.
    let target = 5 * (measured(&inputs[1]) / 16);
    assert!(should_split_chunk(measured(&inputs[1]), target));
    assert!(measured(&inputs[0]) <= target);

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

    // 18 rows over a five-row target pack into four outputs, the first holding the small chunk's
    // two rows plus rows of the big one.
    assert_eq!(outputs.len(), 4);
    assert!(outputs[0].num_rows() > 2);
    for output in &outputs {
        assert!(!should_split_chunk(measured(output), target));
    }
    assert_eq!(outputs.iter().map(|c| c.num_rows()).sum::<usize>(), 18);
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

/// A row-limited cut fills the accumulator's remaining rows the way a byte-limited one fills its
/// remaining bytes: two rows in, a sixteen-row chunk packs as three, five, five, three.
#[test]
fn split_fills_the_row_room() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 8),
        temporal_point_chunk(2, "entity", &(10..26).collect::<Vec<_>>(), 8),
    ];
    let huge = 1024 * 1024 * 1024;

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings(huge, 5)).unwrap());

    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![5, 5, 5, 3]
    );
    assert!(outputs.iter().all(|c| c.all_timelines_sorted()));
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

/// A scalar log: an oversized chunk of one-point rows next to a small one. The piece cut to the
/// accumulator's room fills the output to within a fixed cost of the target, the precision of a
/// fit judged on measured sizes, which for rows this small is a handful of rows out of hundreds.
#[test]
fn room_filling_piece_packs_scalar_rows() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 1], 1),
        temporal_point_chunk(2, "entity", &(10..2010).collect::<Vec<_>>(), 1),
    ];

    // A target worth five hundred rows, and the fixed cost as what a one-row chunk measures.
    let target = measured(&temporal_point_chunk(
        99,
        "entity",
        &(0..500).collect::<Vec<_>>(),
        1,
    ));
    let fixed_cost = measured(&temporal_point_chunk(98, "entity", &[0], 1));
    assert!(should_split_chunk(measured(&inputs[1]), target));

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings(target, 0)).unwrap());

    assert!(
        outputs[0].num_rows() > 400,
        "got {} rows",
        outputs[0].num_rows()
    );
    assert!(measured(&outputs[0]) <= target);
    assert!(
        target - measured(&outputs[0]) <= fixed_cost,
        "{} bytes short of the target, more than a fixed cost of {fixed_cost}",
        target - measured(&outputs[0])
    );
    for output in &outputs {
        assert!(!should_split_chunk(measured(output), target));
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
