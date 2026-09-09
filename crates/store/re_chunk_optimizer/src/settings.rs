use std::num::NonZeroU64;

use re_chunk::{ComponentIdentifier, ComponentType};
use re_log_types::{EntityPathFilter, TimelineName};

/// The knobs of the chunk optimizer.
// TODO(ab): I deliberately growing this type separately from `re_chunk_store::OptimizationProfile`.
// Eventually, it'll supersede it, and might be renamed then. For now, I prefer to keep the name
// distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Components that always get a chunk of their own.
    ///
    /// Rules are tried in order per `(entity, column)`; the first rule whose entity filter and
    /// selector both match decides.
    ///
    /// An all-null temporal column is invisible to the chunk index and stays with the rest of its
    /// chunk.
    pub own_chunk: Vec<OwnChunkRule>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnChunkRule {
    /// If set, which entity path this rule applies to.
    ///
    /// We use an option here to avoid the issue of [`EntityPathFilter::all`] excluding the
    /// `/__properties` subtree.
    pub entity_filter: Option<EntityPathFilter>,

    pub column: ColumnSelector,

    pub merge_split: MergeSplitOverride,
}

impl OwnChunkRule {
    pub fn new(column: ColumnSelector) -> Self {
        Self {
            entity_filter: None,
            column,
            merge_split: MergeSplitOverride::Inherit,
        }
    }

    pub fn with_entity_filter(mut self, entity_filter: EntityPathFilter) -> Self {
        self.entity_filter = Some(entity_filter);
        self
    }
}

/// What selects a component for a chunk of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnSelector {
    /// Every column of this type, whatever its archetype; an untyped column never matches.
    Type(ComponentType),

    /// This column, typed or not.
    Column(ComponentIdentifier),
}

/// How the chunks of one [`OwnChunkRule`] are rechunked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeSplitOverride {
    /// Use [`OptimizationSettings::merge_split`].
    Inherit,

    /// Emit every slice as-is, even when the global setting merges.
    Passthrough,

    /// Use this target instead of the global one, even when the global setting is `None`.
    MergeSplit(MergeSplitSettings),
}
