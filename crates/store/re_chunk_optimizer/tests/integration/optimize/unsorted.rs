//! The row guards for time-unsorted content, `max_rows_if_unsorted`, at merge and at cut time.

use std::sync::Arc;

use re_chunk_optimizer::optimize;
use re_chunk_optimizer::testing::should_split_chunk;

use super::helpers::*;

/// No output ever exceeds the unsorted row guard: a merge of two sorted chunks whose result would
/// be time-unsorted past the guard is discarded, and the operands emit as separate sorted chunks
/// with identity — where legacy admits the flipped merge and ships the over-guard chunk.
#[test]
fn unsorted_merge_past_guard_is_refused() {
    let inputs = vec![
        temporal_point_chunk(1, "entity", &[0, 2, 4, 6], 64),
        temporal_point_chunk(2, "entity", &[1, 3, 5, 7], 64),
    ];
    // RowId order is chunk 1 then chunk 2, so the merge interleaves the time ranges: the result
    // would be 8 time-unsorted rows, past the 4-row unsorted guard.
    let huge = 1024 * 1024 * 1024;

    let outputs = collect(
        optimize(
            provider_of(inputs.clone()),
            &settings_max_row_unsorted(huge, 0, 4),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 2);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
        assert!(output.all_timelines_sorted());
    }

    // With the unsorted guard disabled, the same merge is kept.
    let outputs =
        collect(optimize(provider_of(inputs), &settings_max_row_unsorted(huge, 0, 0)).unwrap());
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].num_rows(), 8);
    assert!(!outputs[0].all_timelines_sorted());
}

/// The unsorted row guard binds at the executor: a run whose merged content is time-unsorted cuts
/// at `max_rows_if_unsorted`; the same content with that guard disabled cuts at `max_rows`; a
/// sorted run is unaffected by a small unsorted guard.
#[test]
fn unsorted_row_guard() {
    // Four sorted two-row chunks whose RowId order interleaves their time ranges: any merge of
    // file-adjacent chunks comes out time-unsorted.
    let unsorted_inputs = || {
        vec![
            temporal_point_chunk(1, "entity", &[10, 11], 64),
            temporal_point_chunk(2, "entity", &[0, 1], 64),
            temporal_point_chunk(3, "entity", &[30, 31], 64),
            temporal_point_chunk(4, "entity", &[20, 21], 64),
        ]
    };
    let huge = 1024 * 1024 * 1024;

    // The merged pairs are unsorted at exactly the guard (4 rows), so they are kept; the unsorted
    // guard then cuts at admission even though the sorted guard is disabled.
    let outputs = collect(
        optimize(
            provider_of(unsorted_inputs()),
            &settings_max_row_unsorted(huge, 0, 4),
        )
        .unwrap(),
    );
    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![4, 4]
    );
    assert!(outputs.iter().all(|c| !c.all_timelines_sorted()));

    // Same content, unsorted guard disabled: `max_rows` applies to every chunk, sorted or not,
    // and here it cuts every chunk apart before any merge.
    let inputs = unsorted_inputs();
    let outputs = collect(
        optimize(
            provider_of(inputs.clone()),
            &settings_max_row_unsorted(huge, 2, 0),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 4);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
    }

    // Same content, both row guards disabled: bytes alone decide, everything merges.
    let outputs = collect(
        optimize(
            provider_of(unsorted_inputs()),
            &settings_max_row_unsorted(huge, 0, 0),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].num_rows(), 8);

    // A sorted run is unaffected by a small unsorted guard.
    let outputs = collect(
        optimize(
            provider_of(vec![
                temporal_point_chunk(1, "entity", &[0, 1], 64),
                temporal_point_chunk(2, "entity", &[10, 11], 64),
            ]),
            &settings_max_row_unsorted(huge, 0, 1),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].num_rows(), 4);
    assert!(outputs[0].all_timelines_sorted());
}

/// An unsorted accumulator switches the cut to `max_rows_if_unsorted`: the piece that joins it
/// brings the merged output exactly to the guard, and no unsorted output ever exceeds it.
#[test]
fn unsorted_accumulator_bounds_the_cut() {
    let unsorted = temporal_point_chunk(1, "entity", &[1, 0], 1024);
    assert!(!unsorted.all_timelines_sorted());
    let inputs = vec![
        unsorted,
        temporal_point_chunk(2, "entity", &(10..26).collect::<Vec<_>>(), 1024),
    ];

    // Eight rows' worth, so the big chunk splits on bytes; the sorted guard is off and the
    // unsorted guard is four rows.
    let target = 8 * (measured(&inputs[1]) / 16);
    assert!(should_split_chunk(measured(&inputs[1]), target));

    let outputs = collect(
        optimize(
            provider_of(inputs.clone()),
            &settings_max_row_unsorted(target, 0, 4),
        )
        .unwrap(),
    );

    assert_eq!(outputs[0].num_rows(), 4);
    assert!(!outputs[0].all_timelines_sorted());
    for output in &outputs[1..] {
        assert!(output.all_timelines_sorted());
    }
    for output in &outputs {
        assert!(!should_split_chunk(measured(output), target));
    }
    assert_eq!(outputs.iter().map(|c| c.num_rows()).sum::<usize>(), 18);
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

/// A lone unsorted chunk over `max_rows_if_unsorted` splits on that guard.
#[test]
fn unsorted_chunk_splits_on_unsorted_guard() {
    let unsorted = temporal_point_chunk(1, "entity", &[10, 0, 30, 20, 5], 64);
    assert!(!unsorted.all_timelines_sorted());
    let huge = 1024 * 1024 * 1024;

    // Rows within the sorted guard but over the unsorted one: legacy splits, and so do we.
    let outputs = collect(
        optimize(
            provider_of(vec![unsorted.clone()]),
            &settings_max_row_unsorted(huge, 8, 2),
        )
        .unwrap(),
    );
    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
    assert_eq!(row_set(std::slice::from_ref(&unsorted)), row_set(&outputs));

    // With the unsorted guard disabled, the same chunk passes through whole.
    let outputs = collect(
        optimize(
            provider_of(vec![unsorted.clone()]),
            &settings_max_row_unsorted(huge, 8, 0),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 1);
    assert!(Arc::ptr_eq(&outputs[0], &unsorted));

    // `max_rows` guards unsorted chunks too: with the unsorted guard disabled or looser, an
    // unsorted chunk over `max_rows` still splits on it, as legacy does.
    for max_rows_if_unsorted in [0, 8] {
        let outputs = collect(
            optimize(
                provider_of(vec![unsorted.clone()]),
                &settings_max_row_unsorted(huge, 2, max_rows_if_unsorted),
            )
            .unwrap(),
        );
        assert_eq!(
            outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
            vec![2, 2, 1]
        );
        assert_eq!(row_set(std::slice::from_ref(&unsorted)), row_set(&outputs));
    }
}
