//! Own-chunk rules: selected columns get chunks of their own, per rule and per entity filter.

use std::collections::BTreeSet;
use std::sync::Arc;

use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_optimizer::testing::should_split_chunk;
use re_chunk_optimizer::{MergeSplitOverride, OptimizationSettings, OwnChunkRule, optimize};
use re_log_types::EntityPathFilter;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_types_core::{ComponentBatch as _, ComponentIdentifier};

use super::helpers::*;

/// With an own-chunk rule, no output holds a selected column next to another one: the colors of
/// every mixed chunk merge into dedicated chunks, the points into the rest.
#[test]
fn own_chunk_separates_columns() {
    let inputs: Vec<Arc<Chunk>> = (0..4)
        .map(|i| temporal_mixed_chunk(i + 1, "entity", &[i as i64 * 10, i as i64 * 10 + 1], 64, 64))
        .collect();
    let settings = with_rules(settings(1024 * 1024, 0), vec![colors_own_chunk_rule()]);

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());

    // One colors chunk and one points chunk.
    assert_eq!(outputs.len(), 2);
    for chunk in &outputs {
        assert!(
            !(chunk.components().contains_component(points())
                && chunk.components().contains_component(colors()))
        );
    }
    let colors_outputs: Vec<_> = outputs
        .iter()
        .filter(|chunk| chunk.components().contains_component(colors()))
        .collect();
    assert_eq!(colors_outputs.len(), 1);
    assert_eq!(colors_outputs[0].components().len(), 1);
    assert_eq!(colors_outputs[0].num_rows(), 8);

    assert_eq!(cell_set(&inputs), cell_set(&outputs));
}

/// Inputs already in the split shape — dedicated colors chunks and points-only chunks, each
/// fitting alone — come out untouched: same `Arc`, same `ChunkId`.
#[test]
fn own_chunk_identity() {
    // Both kinds measure alike: 8 KiB of payload per row.
    let inputs = vec![
        temporal_color_chunk(1, "entity", &[0, 1], 2048),
        temporal_color_chunk(2, "entity", &[10, 11], 2048),
        temporal_point_chunk(3, "entity", &[20, 21], 1024),
        temporal_point_chunk(4, "entity", &[30, 31], 1024),
    ];

    // "Fits alone": above half the target, so no two chunks fit together, at most the target, so
    // no chunk is a band chunk or split.
    let sizes: Vec<u64> = inputs.iter().map(measured).collect();
    let target = *sizes.iter().max().unwrap();
    assert!(sizes.iter().all(|&size| size > target / 2));

    let settings = with_rules(settings(target, 0), vec![colors_own_chunk_rule()]);
    let outputs = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());

    // The colors run emits first, then the rest run: input order by construction.
    assert_eq!(outputs.len(), inputs.len());
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
        assert_eq!(output.id(), input.id());
    }
}

/// Re-optimizing the split output of mixed chunks is a no-op: same chunk count, same `ChunkId`
/// set. Like [`convergence`], with the fixture self-checks run per column set: outputs of
/// different column sets never merge.
#[test]
fn own_chunk_convergence() {
    // Eight mixed chunks whose two slices measure alike: 8 KiB of payload per row either way.
    let inputs: Vec<Arc<Chunk>> = (0..8_u128)
        .map(|i| {
            let times: Vec<i64> = (i as i64 * 10..i as i64 * 10 + 8).collect();
            temporal_mixed_chunk(i + 1, "entity", &times, 1024, 2048)
        })
        .collect();

    // The target is denominated in slices, which is what the runs see.
    let slice_sizes: Vec<u64> = inputs
        .iter()
        .flat_map(|chunk| {
            [
                chunk.components_sliced(&[points()]).total_size_bytes(),
                chunk.components_sliced(&[colors()]).total_size_bytes(),
            ]
        })
        .collect();
    let (min_size, max_size) = (
        *slice_sizes.iter().min().unwrap(),
        *slice_sizes.iter().max().unwrap(),
    );
    // Uniform-ish, by construction: three slices always fit, four never do.
    assert!(4 * min_size > 3 * max_size);
    let target = 3 * max_size;
    let settings = with_rules(settings(target, 0), vec![colors_own_chunk_rule()]);

    let pass_1 = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());
    assert_eq!(cell_set(&inputs), cell_set(&pass_1));

    // Fixture self-checks. Outputs come grouped by column set (the colors run, then the rest
    // run), every output holds one column, and no two adjacent outputs of one column set pairwise
    // fit under the target…
    assert_eq!(
        pass_1.iter().map(|c| columns_of(c)).collect::<Vec<_>>(),
        [
            vec![BTreeSet::from([colors()]); 3],
            vec![BTreeSet::from([points()]); 3]
        ]
        .concat()
    );
    for pair in pass_1.windows(2) {
        if columns_of(&pair[0]) == columns_of(&pair[1]) {
            assert!(measured(&pair[0]) + measured(&pair[1]) > target);
        }
    }

    // …and every pass-1 output measures inside the slack band, so pass 2 splits nothing.
    for chunk in &pass_1 {
        assert!(!should_split_chunk(measured(chunk), target));
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pass1.rrd");
    let store_id = test_store_id();
    write_rrd(&path, &store_id, &pass_1);
    let provider = file_provider(&path, &store_id);

    let pass_2 = collect(optimize(provider, &settings).unwrap());

    assert_eq!(pass_1.len(), pass_2.len());
    let ids = |chunks: &[Arc<Chunk>]| chunks.iter().map(|c| c.id()).collect::<BTreeSet<_>>();
    assert_eq!(ids(&pass_1), ids(&pass_2));
}

/// A rule's own target rechunks its column independently of the global one: the colors run cuts
/// at one chunk's worth of colors while the rest run merges everything.
#[test]
fn own_chunk_run_target() {
    let inputs: Vec<Arc<Chunk>> = (0..4)
        .map(|i| {
            let times: Vec<i64> = (i as i64 * 10..i as i64 * 10 + 4).collect();
            temporal_mixed_chunk(i + 1, "entity", &times, 64, 256)
        })
        .collect();

    // One chunk's colors fit alone under the rule's target; two never do.
    let colors_slice = inputs[0].components_sliced(&[colors()]).total_size_bytes();
    let rule = OwnChunkRule {
        merge_split: MergeSplitOverride::MergeSplit(merge_split_settings(colors_slice)),
        ..colors_own_chunk_rule()
    };
    let settings = with_rules(settings(1024 * 1024 * 1024, 0), vec![rule]);

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());

    let count = |column: ComponentIdentifier| {
        outputs
            .iter()
            .filter(|chunk| chunk.components().contains_component(column))
            .count()
    };
    assert_eq!(count(colors()), 4);
    assert_eq!(count(points()), 1);
    assert_eq!(cell_set(&inputs), cell_set(&outputs));
}

/// A static mixed chunk splits into two static chunks, one per column set.
#[test]
fn own_chunk_static() {
    let input = Arc::new(
        Chunk::builder_with_id(ChunkId::from_u128(1), "entity")
            .with_serialized_batches(
                RowId::from_u128(1 << 32),
                re_log_types::TimePoint::default(),
                [
                    MyPoint::from_iter(0..4)
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap(),
                    MyColor::from_iter(0..4)
                        .try_serialized(MyPoints::descriptor_colors())
                        .unwrap(),
                ],
            )
            .build()
            .unwrap(),
    );
    let settings = with_rules(settings(1024 * 1024, 0), vec![colors_own_chunk_rule()]);

    let outputs = collect(optimize(provider_of(vec![input.clone()]), &settings).unwrap());

    assert_eq!(outputs.len(), 2);
    assert_eq!(
        outputs.iter().map(|c| columns_of(c)).collect::<Vec<_>>(),
        vec![BTreeSet::from([colors()]), BTreeSet::from([points()])]
    );
    for output in &outputs {
        assert!(output.is_static());
        assert_ne!(output.id(), input.id());
    }
    assert_eq!(cell_set(std::slice::from_ref(&input)), cell_set(&outputs));
}

/// A rule applies to the entities its filter names and to no other.
#[test]
fn own_chunk_entity_filter() {
    let inputs = vec![
        temporal_mixed_chunk(1, "split", &[0, 1], 64, 64),
        temporal_mixed_chunk(2, "split", &[10, 11], 64, 64),
        temporal_mixed_chunk(3, "mixed", &[0, 1], 64, 64),
        temporal_mixed_chunk(4, "mixed", &[10, 11], 64, 64),
    ];
    let rule = OwnChunkRule {
        entity_filter: Some(EntityPathFilter::parse_forgiving("+ /split")),
        ..colors_own_chunk_rule()
    };
    let settings = with_rules(settings(1024 * 1024, 0), vec![rule]);

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());

    let columns_by_entity = |entity: &str| {
        outputs
            .iter()
            .filter(|chunk| chunk.entity_path() == &entity.into())
            .map(|chunk| columns_of(chunk))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        columns_by_entity("split"),
        vec![BTreeSet::from([colors()]), BTreeSet::from([points()])]
    );
    assert_eq!(
        columns_by_entity("mixed"),
        vec![BTreeSet::from([points(), colors()])]
    );
    assert_eq!(cell_set(&inputs), cell_set(&outputs));
}

/// With merging disabled, a rule still splits: every mixed chunk yields exactly its two slices,
/// and nothing merges.
#[test]
fn own_chunk_passthrough() {
    let inputs: Vec<Arc<Chunk>> = (0..4)
        .map(|i| temporal_mixed_chunk(i + 1, "entity", &[i as i64 * 10, i as i64 * 10 + 1], 64, 64))
        .collect();
    let settings = OptimizationSettings {
        merge_split: None,
        target_timeline: None,
        own_chunk: vec![colors_own_chunk_rule()],
    };

    let outputs = collect(optimize(provider_of(inputs.clone()), &settings).unwrap());

    assert_eq!(outputs.len(), 2 * inputs.len());
    for output in &outputs {
        assert_eq!(output.components().len(), 1);
        assert_eq!(output.num_rows(), 2);
    }
    assert_eq!(cell_set(&inputs), cell_set(&outputs));
}
