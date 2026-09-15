//! Merge/split over a sequence of uniform chunks, described by row counts.
//!
//! The byte target is what a chunk of `target_rows` rows measures, plus half a row, so a chunk of
//! 24 rows over a 16-row target is 1.5 targets' worth and the output row counts read as whole
//! rows. The half row absorbs the fixed costs the fit counts once per accumulator intermediate,
//! a few hundred bytes each against rows of thirty-two kilobytes.

use re_chunk_optimizer::optimize;
use re_chunk_optimizer::testing::should_split_chunk;

use super::helpers::{collect, measured, provider_of, row_set, settings, temporal_point_chunk};

const POINTS_PER_ROW: u32 = 4096;

/// Run the merge/split over consecutive-time chunks of the given row counts, with the byte
/// target of a `target_rows`-row chunk plus half a row, and no row limits. Returns the output row
/// counts.
///
/// Also checks that no output would be split on a second pass and that every row comes through.
fn sequence(input_rows: &[usize], target_rows: usize) -> Vec<usize> {
    let mut inputs = Vec::with_capacity(input_rows.len());
    let mut first = 0_i64;
    for (i, &rows) in input_rows.iter().enumerate() {
        let end = first + i64::try_from(rows).unwrap();
        let times = (first..end).collect::<Vec<_>>();
        inputs.push(temporal_point_chunk(
            i as u128 + 1,
            "entity",
            &times,
            POINTS_PER_ROW,
        ));
        first = end;
    }

    let reference = |rows: usize| {
        let times = (0..i64::try_from(rows).unwrap()).collect::<Vec<_>>();
        measured(&temporal_point_chunk(
            u128::MAX,
            "entity",
            &times,
            POINTS_PER_ROW,
        ))
    };
    let one_row = reference(target_rows + 1) - reference(target_rows);
    let target = reference(target_rows) + one_row / 2;

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings(target, 0)).unwrap());

    for output in &outputs {
        assert!(
            !should_split_chunk(measured(output), target),
            "an output of {} rows would be split again",
            output.num_rows()
        );
    }
    assert_eq!(row_set(&inputs), row_set(&outputs));
    outputs.iter().map(|chunk| chunk.num_rows()).collect()
}

/// Every chunk over the slack band: each is cut, and its remainder is filled by the next chunk's
/// first piece, so the outputs pack to whole targets with nothing left over.
#[test]
fn all_over_the_band() {
    assert_eq!(sequence(&[20, 20, 20, 20], 16), vec![16, 16, 16, 16, 16]);
}

/// An oversized head followed by chunks of exactly one target: the head's remainder emits alone
/// and the full chunks keep their identity, instead of every chunk being cut to fill the gap.
#[test]
fn oversized_head_then_full_chunks() {
    assert_eq!(
        sequence(&[24, 16, 16, 16, 16], 16),
        vec![16, 8, 16, 16, 16, 16]
    );
}

/// An oversized head followed by the complement of its remainder: the two pack into one output.
#[test]
fn oversized_head_then_half_chunk() {
    assert_eq!(sequence(&[24, 8, 16, 16], 16), vec![16, 16, 16, 16]);
}

/// Baseline: chunks of one target each pass through untouched.
#[test]
fn full_chunks_pass_through() {
    assert_eq!(sequence(&[16, 16, 16], 16), vec![16, 16, 16]);
}
