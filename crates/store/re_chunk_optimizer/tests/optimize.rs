//! End-to-end tests: [`re_chunk_optimizer::optimize`] over an
//! [`re_log_encoding::InMemoryChunkProvider`] or an [`re_log_encoding::RrdChunkProvider`].

#![expect(clippy::unwrap_used)] // `allow-unwrap-in-tests` does not cover helpers in `tests/`

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::executor::block_on;
use futures::{Stream, StreamExt as _, TryStreamExt as _};

use re_byte_size::SizeBytes as _;
use re_chunk::{ArrowArray as _, Chunk, ChunkId, RowId};
use re_chunk_optimizer::testing::{should_split_chunk, smallest_non_splitting_target};
use re_chunk_optimizer::{
    Error, MergeSplitSettings, OptimizationSettings, analyze_chunk_index, optimize,
};
use re_log_encoding::{
    ChunkProvider, ChunkProviderError, EncodingOptions, InMemoryChunkProvider, RawRrdManifest,
    RrdChunkProvider, RrdManifest,
};
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Timeline};
use re_types_core::ComponentBatch as _;

fn temporal_point_chunk(id: u128, entity: &str, times: &[i64], points_per_row: u32) -> Arc<Chunk> {
    let frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &time) in times.iter().enumerate() {
        builder = builder.with_serialized_batches(
            RowId::from_u128((id << 32) + i as u128 + 1),
            [(frame, time)],
            [MyPoint::from_iter(0..points_per_row)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        );
    }
    Arc::new(builder.build().unwrap())
}

fn temporal_color_chunk(id: u128, entity: &str, times: &[i64], colors_per_row: u32) -> Arc<Chunk> {
    let frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &time) in times.iter().enumerate() {
        builder = builder.with_serialized_batches(
            RowId::from_u128((id << 32) + i as u128 + 1),
            [(frame, time)],
            [MyColor::from_iter(0..colors_per_row)
                .try_serialized(MyPoints::descriptor_colors())
                .unwrap()],
        );
    }
    Arc::new(builder.build().unwrap())
}

fn test_store_id() -> StoreId {
    StoreId::new(StoreKind::Recording, "test_app", "test_recording")
}

fn provider_of(chunks: impl IntoIterator<Item = Arc<Chunk>>) -> Arc<InMemoryChunkProvider> {
    Arc::new(InMemoryChunkProvider::new(&test_store_id(), chunks).unwrap())
}

/// Write `chunks` to an RRD file at `path`, in order.
fn write_rrd(path: &Path, store_id: &StoreId, chunks: &[Arc<Chunk>]) {
    let mut file = std::fs::File::create(path).unwrap();
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_log_encoding::CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        &mut file,
    )
    .unwrap();
    encoder
        .append(&LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        }))
        .unwrap();
    for chunk in chunks {
        encoder
            .append(&LogMsg::ArrowMsg(
                store_id.clone(),
                chunk.to_arrow_msg().unwrap(),
            ))
            .unwrap();
    }
    encoder.finish().unwrap();
}

/// Open an RRD file written by [`write_rrd`] as a chunk provider.
fn file_provider(path: &Path, store_id: &StoreId) -> Arc<RrdChunkProvider<std::fs::File>> {
    let footer_file = std::fs::File::open(path).unwrap();
    let footer = block_on(re_log_encoding::read_rrd_footer(&footer_file))
        .unwrap()
        .unwrap();
    let raw = Arc::new(footer.manifests[store_id].clone());
    drop(footer_file);

    let file = std::fs::File::open(path).unwrap();
    Arc::new(RrdChunkProvider::from_reader(file, path.display().to_string(), raw).unwrap())
}

fn collect(stream: impl Stream<Item = Result<Arc<Chunk>, Error>>) -> Vec<Arc<Chunk>> {
    block_on(stream.try_collect()).unwrap()
}

/// Every `(entity, row id)` pair of `chunks`, for row-survival comparisons.
fn row_set(chunks: &[Arc<Chunk>]) -> BTreeSet<(String, RowId)> {
    chunks
        .iter()
        .flat_map(|chunk| {
            let entity = chunk.entity_path().to_string();
            chunk
                .row_ids()
                .map(move |row_id| (entity.clone(), row_id))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn measured(chunk: &Arc<Chunk>) -> u64 {
    chunk.as_ref().total_size_bytes()
}

fn settings_max_row_unsorted(
    max_bytes: u64,
    max_rows: u64,
    max_rows_if_unsorted: u64,
) -> OptimizationSettings {
    OptimizationSettings {
        merge_split: Some(MergeSplitSettings {
            max_bytes: NonZeroU64::new(max_bytes).unwrap(),
            max_rows: NonZeroU64::new(max_rows),
            max_rows_if_unsorted: NonZeroU64::new(max_rows_if_unsorted),
        }),
        target_timeline: None,
    }
}

fn settings(max_bytes: u64, max_rows: u64) -> OptimizationSettings {
    settings_max_row_unsorted(max_bytes, max_rows, 0)
}

#[test]
fn end_to_end() {
    // Four uniform mergeable chunks, one oversized chunk on its own entity.
    let inputs = vec![
        temporal_point_chunk(1, "mergeable", &[0, 1], 64),
        temporal_point_chunk(2, "mergeable", &[10, 11], 64),
        temporal_point_chunk(3, "mergeable", &[20, 21], 64),
        temporal_point_chunk(4, "mergeable", &[30, 31], 64),
        temporal_point_chunk(5, "oversized", &(0..8).collect::<Vec<_>>(), 512),
    ];
    let provider = provider_of(inputs.clone());

    // The executor cuts on measured sizes, so the target is denominated in them.
    let mergeable_size = measured(&inputs[0]);
    for chunk in &inputs[..4] {
        assert_eq!(measured(chunk), mergeable_size, "fixture must be uniform");
    }
    let target = 2 * mergeable_size;

    // The oversized chunk must trip the executor's split gate: measured past the 1.2× slack band.
    assert!(should_split_chunk(measured(&inputs[4]), target));

    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

    // Two merges of two chunks each, and the oversized chunk split into pieces.
    let mergeable_outputs: Vec<_> = outputs
        .iter()
        .filter(|chunk| chunk.entity_path() == &"mergeable".into())
        .collect();
    assert_eq!(mergeable_outputs.len(), 2);
    for chunk in &mergeable_outputs {
        assert_eq!(chunk.num_rows(), 4);
        assert!(measured(chunk) <= target);
    }

    let oversized_outputs: Vec<_> = outputs
        .iter()
        .filter(|chunk| chunk.entity_path() == &"oversized".into())
        .collect();
    assert!(oversized_outputs.len() > 1);
    assert_eq!(
        oversized_outputs
            .iter()
            .map(|chunk| chunk.num_rows())
            .sum::<usize>(),
        8
    );

    // Every row survives.
    assert_eq!(row_set(&inputs), row_set(&outputs));
}

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
    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

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

    let outputs = collect(optimize(provider, settings(1024 * 1024, 0)).unwrap());

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

    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

    // Nothing fits next to the band chunk, so every buffer holds one chunk: three outputs, all
    // identity-preserved, in run order.
    assert_eq!(outputs.len(), 3);
    for (output, input) in std::iter::zip(&outputs, &inputs) {
        assert!(Arc::ptr_eq(output, input));
    }
}

/// Counting wrapper around a provider, to observe when loads happen.
struct CountingProvider {
    inner: Arc<InMemoryChunkProvider>,
    loads: AtomicU64,
}

#[async_trait::async_trait]
impl ChunkProvider for CountingProvider {
    fn manifest(&self) -> &Arc<RrdManifest> {
        self.inner.manifest()
    }

    fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        self.inner.raw_manifest()
    }

    fn source(&self) -> String {
        self.inner.source()
    }

    async fn load_chunks(&self, ids: &[ChunkId]) -> Result<Vec<Arc<Chunk>>, ChunkProviderError> {
        self.loads.fetch_add(ids.len() as u64, Ordering::Relaxed);
        self.inner.load_chunks(ids).await
    }
}

#[test]
fn lazy_loading() {
    let inputs = vec![
        temporal_point_chunk(1, "a", &[0, 1], 64),
        temporal_point_chunk(2, "b", &[0, 1], 64),
        temporal_point_chunk(3, "c", &[0, 1], 64),
    ];
    let provider = Arc::new(CountingProvider {
        inner: provider_of(inputs),
        loads: AtomicU64::new(0),
    });

    // Planning does no IO.
    let stream = optimize(Arc::clone(&provider) as _, settings(1024 * 1024, 0)).unwrap();
    assert_eq!(provider.loads.load(Ordering::Relaxed), 0);

    // Loads spread across the stream's polls: three lone entities, one load each.
    futures::pin_mut!(stream);
    let mut seen_loads = Vec::new();
    while let Some(chunk) = block_on(stream.next()) {
        chunk.unwrap();
        seen_loads.push(provider.loads.load(Ordering::Relaxed));
    }
    assert_eq!(seen_loads, vec![1, 2, 3]);
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
    let outputs = collect(optimize(provider, settings(1024 * 1024, 0)).unwrap());

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
    let outputs = collect(optimize(provider, settings(1024 * 1024, 0)).unwrap());

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
    let outputs = collect(optimize(provider, settings(1024 * 1024, 0)).unwrap());

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
            settings_max_row_unsorted(huge, 0, 4),
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
        collect(optimize(provider_of(inputs), settings_max_row_unsorted(huge, 0, 0)).unwrap());
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
            settings_max_row_unsorted(huge, 0, 4),
        )
        .unwrap(),
    );
    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![4, 4]
    );
    assert!(outputs.iter().all(|c| !c.all_timelines_sorted()));

    // Same content, unsorted guard disabled: the sorted guard applies only while everything at
    // the gate is still sorted — here it cuts every chunk apart before any merge.
    let inputs = unsorted_inputs();
    let outputs = collect(
        optimize(
            provider_of(inputs.clone()),
            settings_max_row_unsorted(huge, 2, 0),
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
            settings_max_row_unsorted(huge, 0, 0),
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
            settings_max_row_unsorted(huge, 0, 1),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].num_rows(), 4);
    assert!(outputs[0].all_timelines_sorted());
}

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

    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

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

    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

    // The one-row tail piece coalesces with the following one-row chunk.
    assert_eq!(
        outputs.iter().map(|c| c.num_rows()).collect::<Vec<_>>(),
        vec![3, 3, 2]
    );
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
            settings_max_row_unsorted(huge, 8, 2),
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
            settings_max_row_unsorted(huge, 8, 0),
        )
        .unwrap(),
    );
    assert_eq!(outputs.len(), 1);
    assert!(Arc::ptr_eq(&outputs[0], &unsorted));
}

/// The number of bins a plan-time first-fit sweep over `sizes` produces at `target`.
fn first_fit_bins(sizes: &[u64], target: u64) -> u64 {
    let mut bins = 0_u64;
    let mut bin_bytes = u64::MAX; // force an open on the first chunk
    for &size in sizes {
        if bin_bytes.saturating_add(size) > target {
            bins += 1;
            bin_bytes = 0;
        }
        bin_bytes += size;
    }
    bins
}

/// Compaction of tiny chunk is not affected by the per-chunk frame overhead.
#[test]
fn tiny_chunks_file() {
    const NUM_CHUNKS: u64 = 64;

    let store_id = test_store_id();
    let chunks: Vec<Arc<Chunk>> = (0..NUM_CHUNKS)
        .map(|i| temporal_point_chunk(u128::from(i) + 1, "tiny", &[i.cast_signed()], 512))
        .collect();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny.rrd");
    write_rrd(&path, &store_id, &chunks);

    // The data floor: everything merged into one chunk, per-chunk constants fully collapsed.
    let merged_all =
        collect(optimize(file_provider(&path, &store_id), settings(u64::MAX / 2, 0)).unwrap());
    assert_eq!(merged_all.len(), 1);
    let total_data = measured(&merged_all[0]);

    // A target a few chunks' measured data wide.
    let target = total_data.div_ceil(4);
    let floor = total_data.div_ceil(target);

    let provider = file_provider(&path, &store_id);

    // What per-chunk-summing gates would produce at this target: a first-fit sweep over the
    // index's charged bytes (the deleted plan-time binning), and one over the decoded chunks'
    // unmerged measurements (a buffer-of-chunks running sum).
    let charged: Vec<u64> = provider
        .raw_manifest()
        .col_chunk_byte_size_uncompressed()
        .unwrap()
        .to_vec();
    let charged_bins = first_fit_bins(&charged, target);
    let unmerged_sum_bins = {
        let ids = provider.manifest().col_chunk_ids().to_vec();
        let decoded = block_on(provider.load_chunks(&ids)).unwrap();
        let sizes: Vec<u64> = decoded.iter().map(measured).collect();
        first_fit_bins(&sizes, target)
    };

    let outputs = collect(optimize(provider, settings(target, 0)).unwrap());

    // One run reaches the data floor, and beats both per-chunk-summing gates.
    assert!(
        (outputs.len() as u64).abs_diff(floor) <= 1,
        "outputs: {}, floor: {floor}",
        outputs.len()
    );
    assert!((outputs.len() as u64) < charged_bins);
    assert!((outputs.len() as u64) < unmerged_sum_bins);
    assert_eq!(row_set(&chunks), row_set(&outputs));
}

/// Re-optimizing already-optimized output is a no-op: same chunk count, same `ChunkId` set.
///
/// The fixture includes a heterogeneous-component group (union padding, absorbed by the slack
/// band) and sizes its chunks uniformly enough that no two adjacent pass-1 outputs pairwise fit
/// under the target (a fitting tail runt would legitimately coalesce on pass 2).
#[test]
fn convergence() {
    // Heterogeneous group: alternating points and colors chunks of near-equal measured size
    // (8 KiB per row either way). Homogeneous group: five points chunks.
    let mut inputs: Vec<Arc<Chunk>> = Vec::new();
    for i in 0..8_u128 {
        let times: Vec<i64> = (i as i64 * 10..i as i64 * 10 + 8).collect();
        inputs.push(if i % 2 == 0 {
            temporal_point_chunk(i + 1, "hetero", &times, 1024)
        } else {
            temporal_color_chunk(i + 1, "hetero", &times, 2048)
        });
    }
    for i in 0..5_u128 {
        let times: Vec<i64> = (i as i64 * 10..i as i64 * 10 + 8).collect();
        inputs.push(temporal_point_chunk(100 + i, "homogeneous", &times, 1024));
    }

    let sizes: Vec<u64> = inputs.iter().map(measured).collect();
    let (min_size, max_size) = (*sizes.iter().min().unwrap(), *sizes.iter().max().unwrap());
    // Uniform-ish, by construction: three chunks always fit, four never do.
    assert!(4 * min_size > 3 * max_size);
    let target = 3 * max_size;

    let pass_1 = collect(optimize(provider_of(inputs.clone()), settings(target, 0)).unwrap());
    assert_eq!(row_set(&inputs), row_set(&pass_1));

    // Fixture self-checks. No two adjacent pass-1 outputs pairwise fit under the target…
    for pair in pass_1.windows(2) {
        if pair[0].entity_path() == pair[1].entity_path() {
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

    let pass_2 = collect(optimize(provider, settings(target, 0)).unwrap());

    assert_eq!(pass_1.len(), pass_2.len());
    let ids = |chunks: &[Arc<Chunk>]| chunks.iter().map(|c| c.id()).collect::<BTreeSet<_>>();
    assert_eq!(ids(&pass_1), ids(&pass_2));
}
