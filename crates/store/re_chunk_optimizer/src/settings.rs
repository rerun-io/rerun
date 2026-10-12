use std::num::NonZeroU64;

use re_chunk::{ComponentIdentifier, ComponentType};
use re_log_types::{EntityPathFilter, TimelineName};

/// The knobs of the chunk optimizer.
// TODO(ab): I deliberately growing this type separately from `re_chunk_store::OptimizationProfile`.
// Eventually, it'll supersede it, and might be renamed then. For now, I prefer to keep the name
// distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimizationSettings {
    /// Merge and split chunks toward a target size; `None` disables the optimization and every
    /// chunk passes through, except for the video streams matched by [`Self::gop_batching`].
    pub merge_split: Option<MergeSplitSettings>,

    /// Timeline that orders the merge sweep; `None` means file order.
    ///
    /// Chunks are swept whole, ordered by their time range on this timeline; rows are never
    /// reordered. A group of chunks that lacks the timeline falls back to file order silently,
    /// and a name that matches no timeline in the recording means every group falls back.
    ///
    /// GoP-batched video streams are the exception: their rows are sorted on this timeline when
    /// they have it, else on the best of their own timelines.
    pub target_timeline: Option<TimelineName>,

    /// Components that always get a chunk of their own.
    ///
    /// Rules are tried in order per `(entity, column)`; the first rule whose entity filter and
    /// selector both match decides.
    ///
    /// An all-null temporal column is invisible to the chunk index and stays with the rest of its
    /// chunk.
    ///
    /// Rules selecting `VideoStream:sample`, `VideoStream:codec`, or `VideoStream:is_keyframe` are
    /// ignored on entities matched by [`Self::gop_batching`].
    pub own_chunk: Vec<OwnChunkRule>,

    /// Video streams whose chunks are cut at keyframes, without ever splitting a GoP.
    ///
    /// Rules are tried in order per entity; the first whose entity filter matches decides. Only
    /// entities with a temporal `VideoStream:sample` column are considered. One without a
    /// `VideoStream:codec` column, or whose samples have inconsistent sets of timeline, is skipped
    /// with a warning.
    ///
    /// Keyframes are detected from the samples: logged `VideoStream:is_keyframe` cells are dropped
    /// and regenerated. A sample the detector rejects counts as a non-keyframe, with a warning.
    ///
    /// Each stream comes out with its codec as one static `VideoStream:codec` cell. A static codec
    /// wins over temporal ones, as in the store; temporal codecs that disagree are unsupported and
    /// fail the whole optimization.
    pub gop_batching: Vec<GopBatchingRule>,
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

    /// Row guard for every chunk; `None` disables it.
    pub max_rows: Option<NonZeroU64>,

    /// Tighter row guard for chunks with at least one unsorted timeline, applied on top of
    /// [`Self::max_rows`]; `None` disables it.
    pub max_rows_if_unsorted: Option<NonZeroU64>,
}

impl MergeSplitSettings {
    /// The row guard for content whose timelines are all sorted, or for content that is not:
    /// unsorted content is held to the tighter of the two guards.
    pub fn row_guard(&self, sorted: bool) -> Option<NonZeroU64> {
        if sorted {
            self.max_rows
        } else {
            match (self.max_rows, self.max_rows_if_unsorted) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (guard, None) | (None, guard) => guard,
            }
        }
    }
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

/// How the chunks of one [`OwnChunkRule`] or [`GopBatchingRule`] are rechunked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeSplitOverride {
    /// Use [`OptimizationSettings::merge_split`].
    Inherit,

    /// Emit every slice as-is, even when the global setting merges.
    Passthrough,

    /// Use this target instead of the global one, even when the global setting is `None`.
    MergeSplit(MergeSplitSettings),
}

/// GoP batching of the video streams of the entities the filter matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GopBatchingRule {
    /// If set, which entity path this rule applies to.
    ///
    /// We use an option here to avoid the issue of [`EntityPathFilter::all`] excluding the
    /// `/__properties` subtree.
    pub entity_filter: Option<EntityPathFilter>,

    /// How whole GoPs pack into sample chunks: `Passthrough` is one chunk per GoP.
    pub samples: MergeSplitOverride,

    /// Where the regenerated `is_keyframe` column goes.
    pub keyframes: KeyframePlacement,
}

impl Default for GopBatchingRule {
    fn default() -> Self {
        Self {
            entity_filter: None,
            samples: MergeSplitOverride::Inherit,
            keyframes: KeyframePlacement::OwnChunk(None),
        }
    }
}

impl GopBatchingRule {
    pub fn with_entity_filter(mut self, entity_filter: EntityPathFilter) -> Self {
        self.entity_filter = Some(entity_filter);
        self
    }
}

/// Where a [`GopBatchingRule`] puts the `is_keyframe` column it regenerates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframePlacement {
    /// A column of the sample chunks: `true` on keyframe rows, null elsewhere.
    Inline,

    /// Chunks of its own, so readers can find the samples they need without loading the sample
    /// chunks.
    ///
    /// Packed toward this target, or toward [`OptimizationSettings::merge_split`] when `None`;
    /// with neither, each stream's `is_keyframe` cells form a single chunk.
    ///
    /// Unlike [`MergeSplitOverride`], there is no passthrough: the cells are generated, one per
    /// GoP, so there are no input chunks to pass through.
    OwnChunk(Option<MergeSplitSettings>),
}
