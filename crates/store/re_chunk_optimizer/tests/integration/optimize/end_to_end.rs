//! Whole-pipeline properties: mixed inputs, lazy loading, real files, and idempotency.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::StreamExt as _;
use futures::executor::block_on;

use re_chunk::{Chunk, ChunkId};
use re_chunk_index::{
    ChunkProvider, ChunkProviderError, InMemoryChunkProvider, RawRrdManifest, RrdManifest,
};
use re_chunk_optimizer::optimize;
use re_chunk_optimizer::testing::should_split_chunk;

use super::helpers::*;

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

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

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
    let stream = optimize(Arc::clone(&provider) as _, &settings(1024 * 1024, 0)).unwrap();
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
        collect(optimize(file_provider(&path, &store_id), &settings(u64::MAX / 2, 0)).unwrap());
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

    let outputs = collect(optimize(provider, &settings(target, 0)).unwrap());

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

    let pass_1 = collect(optimize(provider_of(inputs.clone()), &settings(target, 0)).unwrap());
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

    let pass_2 = collect(optimize(provider, &settings(target, 0)).unwrap());

    assert_eq!(pass_1.len(), pass_2.len());
    let ids = |chunks: &[Arc<Chunk>]| chunks.iter().map(|c| c.id()).collect::<BTreeSet<_>>();
    assert_eq!(ids(&pass_1), ids(&pass_2));
}
