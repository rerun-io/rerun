//! Chunk index analysis: assess a recording based on its chunk index.
//!
//! The analysis focuses on chunk size, assessing the recording against a provided chunk size
//! target. The row limit as well as optimizations (e.g. GoP batching, thick/thin splitting, static
//! dedup) are not considered (yet). In practice, they would correlate strongly with merging.
//!
//! The analysis has two steps.
//!
//! **1. A lower bound on the achievable chunk count** ([`MergeAssessment::achievable_chunks`]).
//! Merging never crosses entity, timeline-set, or static/temporal boundaries, and no output chunk
//! may exceed the byte target. So, per group of mergeable chunks — one entity, one exact timeline
//! set, temporal chunks only — the bound is:
//!
//! ```text
//! group_achievable = clamp(ceil(group_bytes / chunk_max_bytes), 1, group_chunks)
//! ```
//!
//! Note: Component sets do not gate merging (merged chunks union their columns), so they do not
//! split groups either.
//!
//! **2. A trivial heuristic on the excess** ([`MergeAssessment::looks_unoptimized`]). The
//! recording "looks unoptimized" when two gates both trip: the factor `actual / achievable`
//! reaches [`UNOPTIMIZED_FACTOR_THRESHOLD`], and the excess `actual − achievable` reaches
//! [`UNOPTIMIZED_EXCESS_THRESHOLD`] chunks. The ratio catches bad packing; the absolute gate
//! keeps small recordings silent.

use re_log_encoding::RawRrdManifest;

use crate::error::Error;
use crate::view::ChunkIndexView;

/// Everything this crate can tell about one store's chunk index.
#[derive(Clone, Debug)]
pub struct ChunkIndexAnalysis {
    /// Should (and could) the recording be optimized
    pub merge: MergeAssessment,

    /// The number of columns of the chunk index itself.
    pub num_columns: usize,
}

/// Analyzes one store's chunk index.
///
/// `chunk_max_bytes` is the merge byte target the assessment measures against (pass e.g.
/// `re_chunk_store::OptimizationProfile::OBJECT_STORE.chunk_max_bytes`).
pub fn analyze_chunk_index(
    chunk_index: &RawRrdManifest,
    chunk_max_bytes: u64,
) -> Result<ChunkIndexAnalysis, Error> {
    let view = ChunkIndexView::try_from_raw(chunk_index)?;
    Ok(ChunkIndexAnalysis {
        merge: MergeAssessment::compute(&view, chunk_max_bytes),
        num_columns: view.num_columns,
    })
}

/// [`MergeAssessment::looks_unoptimized`] requires at least this many times more chunks than
/// achievable. See the module documentation.
const UNOPTIMIZED_FACTOR_THRESHOLD: f64 = 2.0;

/// [`MergeAssessment::looks_unoptimized`] requires at least this many chunks in excess of
/// achievable. See the module documentation.
const UNOPTIMIZED_EXCESS_THRESHOLD: u64 = 200;

/// Whether a recording's chunk count is close to what merging could achieve.
#[derive(Clone, Copy, Debug, Default)]
pub struct MergeAssessment {
    /// Temporal chunks in the chunk index. Merging never touches static chunks.
    pub actual_chunks: u64,

    /// A lower bound on the chunk count that merging could reach.
    ///
    /// See the module documentation for how it is computed.
    pub achievable_chunks: u64,

    /// `actual_chunks − achievable_chunks`.
    pub excess_chunks: u64,

    /// `actual_chunks / achievable_chunks`. `1.0` when there is nothing to merge.
    ///
    /// Over-states how much merging can help, since the denominator is a lower bound; see the
    /// module documentation.
    pub factor: f64,
}

impl MergeAssessment {
    pub fn compute(view: &ChunkIndexView, chunk_max_bytes: u64) -> Self {
        let chunk_max_bytes = chunk_max_bytes.max(1);

        let mut actual_chunks = 0u64;
        let mut achievable_chunks = 0u64;

        for entity in view.entities.values() {
            for group in &entity.timeline_sets {
                let group_chunks = group.num_chunks() as u64;
                let group_bytes: u64 = group
                    .per_timeline
                    .values()
                    .next()
                    .into_iter()
                    .flatten()
                    .map(|span| view.chunk(span.chunk).byte_size_uncompressed)
                    .sum();

                // A single chunk over the target is legal (e.g. one GoP), hence the upper clamp.
                let group_achievable = group_bytes
                    .div_ceil(chunk_max_bytes)
                    .clamp(1, group_chunks.max(1));

                actual_chunks += group_chunks;
                achievable_chunks += group_achievable.min(group_chunks);
            }
        }

        Self {
            actual_chunks,
            achievable_chunks,
            excess_chunks: actual_chunks - achievable_chunks,
            factor: if achievable_chunks == 0 {
                1.0
            } else {
                actual_chunks as f64 / achievable_chunks as f64
            },
        }
    }

    /// Whether this recording should be optimized before it is registered.
    ///
    /// True when both gates of the module documentation trip: factor ≥ 2× and excess ≥ 200.
    pub fn looks_unoptimized(&self) -> bool {
        self.looks_unoptimized_with(UNOPTIMIZED_FACTOR_THRESHOLD, UNOPTIMIZED_EXCESS_THRESHOLD)
    }

    /// [`Self::looks_unoptimized`] with explicit thresholds.
    pub fn looks_unoptimized_with(&self, factor_threshold: f64, excess_threshold: u64) -> bool {
        self.factor >= factor_threshold && self.excess_chunks >= excess_threshold
    }
}
