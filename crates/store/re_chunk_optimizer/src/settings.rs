use std::num::NonZeroU64;

use re_log_types::TimelineName;

/// The knobs of the chunk optimizer.
// TODO(ab): I deliberately growing this type separately from `re_chunk_store::OptimizationProfile`.
// Eventually, it'll supersede it, and might be renamed then. For now, I prefer to keep the name
// distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimizationSettings {
    /// Merge and split chunks toward a target size; `None` disables the optimization entirely and
    /// every chunk passes through.
    pub merge_split: Option<MergeSplitSettings>,

    /// Timeline that orders the merge sweep; `None` means file order.
    ///
    /// Chunks are swept whole, ordered by their time range on this timeline; rows are never
    /// reordered. A group of chunks that lacks the timeline falls back to file order silently,
    /// and a name that matches no timeline in the recording means every group falls back.
    ///
    /// Only read when [`Self::merge_split`] is `Some`.
    pub target_timeline: Option<TimelineName>,
}

/// The single vertical optimization: rechunk rows toward a byte target, with row-count guards on
/// that target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeSplitSettings {
    /// Byte target for output chunks.
    ///
    /// The executor cuts output boundaries and splits oversized chunks on the measured size
    /// (heap bytes) of the decoded chunks it holds, with a slack band (`1.2 ×` this target) that
    /// keeps re-optimization of already-optimized chunks a no-op.
    pub max_bytes: NonZeroU64,

    /// Row guard for chunks whose timelines are all sorted; `None` disables it.
    pub max_rows: Option<NonZeroU64>,

    /// Row guard for chunks with at least one unsorted timeline; `None` disables it.
    pub max_rows_if_unsorted: Option<NonZeroU64>,
}
