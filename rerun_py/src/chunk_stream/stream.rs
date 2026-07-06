//! Pipeline description types for the chunk pipeline.
//!
//! This module contains the declarative, user-facing types that describe a lazy chunk pipeline:
//! [`StructuredFilter`], [`PipelineStep`], [`StreamSource`], and [`LazyChunkStream`] itself.
//! We model this as a single-sink DAG, aka the API user holds on the DAG by the sink node, which is
//! then executed into a materialized collection of chunk (RRD, `list[Chunk]`, etc.).
//!
//! Execution happens in [`super::engine`], which is reached through
//! [`LazyChunkStream::compile`].

use std::sync::Arc;

use pyo3::{Py, PyAny, Python};

use re_chunk::Chunk;
use re_types_core::ComponentIdentifier;

use super::{ChunkStream, ChunkStreamFactory};

/// Declarative, composable filter for chunks.
///
/// Contains two kinds of criteria, combined with AND:
///
/// **Chunk-level predicates** (`content`, `has_timeline`, `is_static`): the chunk
/// either passes or is dropped entirely. The chunk data is never modified.
///
/// **Column-level selector** (`components`): the chunk is split by component
/// columns. Matching columns are separated from non-matching ones, producing
/// a new chunk with a subset of the original's columns (entity path and timelines
/// are always preserved). When multiple components are listed, any column matching
/// any of them is kept (OR semantics).
///
/// When both kinds are combined, predicates are evaluated first. If any predicate
/// fails, the chunk is dropped entirely — no column splitting occurs.
#[derive(Clone, Debug, Default)]
pub struct StructuredFilter {
    /// Entity path filter (include/exclude rules with subtree matching).
    pub content: Option<re_log_types::ResolvedEntityPathFilter>,

    /// Chunk must have a column for this timeline.
    pub has_timeline: Option<re_types_core::TimelineName>,

    /// `true` -> static only; `false` -> temporal only; `None` -> both.
    pub is_static: Option<bool>,

    /// Keep only component columns whose [`ComponentIdentifier`] appears in this list.
    ///
    /// OR semantics: a chunk passes if it contains *any* of the listed components.
    /// Only matching columns are kept; the rest are stripped.
    pub components: Option<Vec<ComponentIdentifier>>,
}

/// Outcome of [`StructuredFilter::try_merge`].
//TODO(RR-4717): Why do we need this? Because `StructuredFilter`'s representation is not general
// enough to express all possible merges. We should explore making it more general, such that we
// can simplify the try_merge API and, possibly, make the pushdown filtering slightly more powerful.
#[derive(Debug)]
pub enum MergeResult {
    /// Successfully merged into a single filter.
    Merged(StructuredFilter),

    /// Some field couldn't be combined; keep `other` as a separate post-filter.
    Conflict,

    /// AND is unsatisfiable (e.g. `is_static=true AND is_static=false`).
    Empty,
}

/// View over the chunk-level metadata that a [`StructuredFilter`] inspects.
///
/// This is used such that the same [`StructuredFilter::matches`] code path can be applied
/// to both actual chunk filtering, and manifest filtering during pushdown.
pub(super) trait ChunkPredicateView {
    fn entity_path(&self) -> &re_log_types::EntityPath;
    fn is_static(&self) -> bool;
    fn has_timeline(&self, name: &re_types_core::TimelineName) -> bool;
    fn has_any_component(&self, components: &[ComponentIdentifier]) -> bool;
}

impl ChunkPredicateView for Chunk {
    fn entity_path(&self) -> &re_log_types::EntityPath {
        Self::entity_path(self)
    }

    fn is_static(&self) -> bool {
        Self::is_static(self)
    }

    fn has_timeline(&self, name: &re_types_core::TimelineName) -> bool {
        self.timelines().contains_key(name)
    }

    fn has_any_component(&self, components: &[ComponentIdentifier]) -> bool {
        components
            .iter()
            .any(|c| self.components().contains_component(*c))
    }
}

impl StructuredFilter {
    /// AND-merge `other` into `self`.
    ///
    /// Per-field semantics when both sides are `Some` (when one side is `None`, the result is
    /// always whichever side is `Some`):
    ///
    /// | Field          | Both `Some`                                                   |
    /// |----------------|---------------------------------------------------------------|
    /// | `content`      | equal → take it; different → [`MergeResult::Conflict`]        |
    /// | `has_timeline` | same name → take it; different → [`MergeResult::Conflict`]    |
    /// | `is_static`    | same → take it; different → [`MergeResult::Empty`]            |
    /// | `components`   | intersect; empty intersection → [`MergeResult::Empty`]        |
    ///
    /// `content` falls back to `Conflict` (rather than `Empty`) when the two filters differ
    /// because [`re_log_types::ResolvedEntityPathFilter`] uses a specificity-ordered rule set
    /// ("most specific match wins"); intersecting two such rule sets is not concatenation and
    /// cannot be computed structurally. Equal filters are trivially their own intersection.
    pub fn try_merge(&self, other: &Self) -> MergeResult {
        //TODO(ab): in theory we should be able to merge strictly overlapping contents
        let content = match (&self.content, &other.content) {
            (Some(a), Some(b)) => {
                if a == b {
                    Some(a.clone())
                } else {
                    return MergeResult::Conflict;
                }
            }
            (Some(c), None) | (None, Some(c)) => Some(c.clone()),
            (None, None) => None,
        };

        let has_timeline = match (self.has_timeline, other.has_timeline) {
            (Some(a), Some(b)) => {
                if a == b {
                    Some(a)
                } else {
                    return MergeResult::Conflict;
                }
            }
            (Some(t), None) | (None, Some(t)) => Some(t),
            (None, None) => None,
        };

        let is_static = match (self.is_static, other.is_static) {
            (Some(a), Some(b)) => {
                if a == b {
                    Some(a)
                } else {
                    return MergeResult::Empty;
                }
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };

        let components = match (&self.components, &other.components) {
            (Some(a), Some(b)) => {
                let intersection: Vec<_> = a.iter().copied().filter(|c| b.contains(c)).collect();
                if intersection.is_empty() {
                    return MergeResult::Empty;
                }
                Some(intersection)
            }
            (Some(c), None) | (None, Some(c)) => Some(c.clone()),
            (None, None) => None,
        };

        MergeResult::Merged(Self {
            content,
            has_timeline,
            is_static,
            components,
        })
    }

    /// `true` iff this filter has no predicates and no component selection,
    /// i.e. `apply(c)` always returns `Some(c)` unchanged.
    pub fn is_noop(&self) -> bool {
        self.content.is_none()
            && self.has_timeline.is_none()
            && self.is_static.is_none()
            && self.components.is_none()
    }

    /// Check chunk-level predicates (content, has_timeline, is_static) against `view`.
    /// Does NOT check the `components` clause — callers like [`Self::apply_complement`] and
    /// [`Self::split`] distinguish "predicate failed" from "predicate passed but components
    /// don't match", so they need the predicates-only answer.
    fn predicates_match(&self, view: &impl ChunkPredicateView) -> bool {
        // Destructure so adding a new predicate field forces a compile error here.
        let Self {
            content,
            has_timeline,
            is_static,
            components: _,
        } = self;

        if let Some(c) = content
            && !c.matches(view.entity_path())
        {
            return false;
        }
        if let Some(want) = is_static
            && view.is_static() != *want
        {
            return false;
        }
        if let Some(tl) = has_timeline
            && !view.has_timeline(tl)
        {
            return false;
        }
        true
    }

    /// Full match: predicates AND components clause.
    ///
    /// Single source of truth for "does this chunk pass the filter as a whole?" — used by
    /// both [`Self::apply`] (post-load) and `evaluate_filter_on_manifest` (pre-load
    /// pushdown). Returning `true` here means the chunk survives filtering; the caller is
    /// responsible for any column slicing implied by `components`.
    pub(super) fn matches(&self, view: &impl ChunkPredicateView) -> bool {
        // Destructure so adding a new field forces a compile error here.
        let Self {
            content,
            has_timeline,
            is_static,
            components,
        } = self;

        if let Some(c) = content
            && !c.matches(view.entity_path())
        {
            return false;
        }
        if let Some(want) = is_static
            && view.is_static() != *want
        {
            return false;
        }
        if let Some(tl) = has_timeline
            && !view.has_timeline(tl)
        {
            return false;
        }
        if let Some(comps) = components
            && !view.has_any_component(comps)
        {
            return false;
        }
        true
    }

    /// For `filter()`: returns the matching portion of the chunk, or `None`.
    ///
    /// When no component filter is set and the chunk passes all predicates,
    /// the original `Arc` is returned as-is (zero-cost move).
    pub fn apply(&self, chunk: Arc<Chunk>) -> Option<Arc<Chunk>> {
        if !self.matches(&*chunk) {
            return None;
        }

        if let Some(components) = &self.components {
            Some(Arc::new(chunk.components_sliced(components)))
        } else {
            Some(chunk)
        }
    }

    /// For `drop()`: returns the complement -- what `apply()` would discard.
    ///
    /// When predicates don't match (chunk is kept entirely), the original `Arc`
    /// is returned as-is (zero-cost move).
    pub fn apply_complement(&self, chunk: Arc<Chunk>) -> Option<Arc<Chunk>> {
        if !self.predicates_match(&*chunk) {
            return Some(chunk);
        }

        if let Some(ref components) = self.components {
            let dropped = chunk.components_dropped(components);
            if dropped.num_components() == 0 {
                None
            } else {
                Some(Arc::new(dropped))
            }
        } else {
            None
        }
    }

    /// For `split()`: returns `(matching, complement)`. Either may be `None`.
    ///
    /// When no component filter is set and the chunk passes all predicates,
    /// the original `Arc` is moved to the matching side (zero-cost).
    pub fn split(&self, chunk: Arc<Chunk>) -> (Option<Arc<Chunk>>, Option<Arc<Chunk>>) {
        if !self.predicates_match(&*chunk) {
            return (None, Some(chunk));
        }

        if let Some(ref components) = self.components {
            let selected = chunk.components_sliced(components);
            let dropped = chunk.components_dropped(components);

            let matching = if selected.num_components() > 0 {
                Some(Arc::new(selected))
            } else {
                None
            };
            let non_matching = if dropped.num_components() > 0 {
                Some(Arc::new(dropped))
            } else {
                None
            };

            (matching, non_matching)
        } else {
            (Some(chunk), None)
        }
    }
}

/// A single transformation step in the pipeline.
pub enum PipelineStep {
    Filter(StructuredFilter),
    Drop(StructuredFilter),
    Lenses {
        lenses: re_lenses_core::Lenses,

        /// Optional content filter: when set, lenses are applied only to chunks
        /// whose entity path matches; non-matching chunks pass through unchanged.
        content: Option<re_log_types::ResolvedEntityPathFilter>,
    },
    Map(Py<PyAny>),
    FlatMap(Py<PyAny>),
}

impl Clone for PipelineStep {
    fn clone(&self) -> Self {
        match self {
            Self::Filter(f) => Self::Filter(f.clone()),
            Self::Drop(f) => Self::Drop(f.clone()),
            Self::Lenses { lenses, content } => Self::Lenses {
                lenses: lenses.clone(),
                content: content.clone(),
            },
            Self::Map(c) => Self::Map(Python::attach(|py| c.clone_ref(py))),
            Self::FlatMap(c) => Self::FlatMap(Python::attach(|py| c.clone_ref(py))),
        }
    }
}

/// The source of chunks for a pipeline.
pub enum StreamSource {
    /// A stream factory, e.g. a reader that produces chunks from a file.
    StreamFactory(Box<dyn ChunkStreamFactory>),

    /// Wrap a Python iterable of Chunks.
    ///
    /// **WARNING**: this is a problematic construct. Python iterator _are_ iterable (their
    /// `__iter__` method typically return `self`), and still one-shot. It means that if a
    /// `PyIterator` source is eventually compiled and executed twice, it will yield no data on
    /// the second execution.
    //TODO(RR-4265): should this be a iterator _factory_ instead?
    //TODO(ab): abstract this behind `StreamFactory`
    PyIterable(Py<PyAny>),

    /// Concatenate multiple streams.
    Merged(Vec<LazyChunkStream>),

    /// One branch of a split.
    ///
    /// Both branches of the same split share one [`Arc<SplitOrigin>`]. The execution engine
    /// is responsible for compiling the upstream once and routing chunks to the appropriate
    /// branch; see [`super::engine`] for the runtime machinery.
    SplitBranch {
        origin: Arc<SplitOrigin>,
        side: SplitSide,
    },
}

/// Which side of a split is this?
///
/// Splits are based on a user-provided [`StructuredFilter`]. The matched data is routed to the
/// first branch ([`Self::Matched`]), and the rest is routed to the second branch
/// ([`Self::Unmatched`]).
#[derive(Clone, Copy, Debug)]
pub enum SplitSide {
    Matched,
    Unmatched,
}

/// Description of a split operation: what stream is split, and according to what filter.
pub struct SplitOrigin {
    pub upstream: LazyChunkStream,
    pub filter: StructuredFilter,
}

/// Lazy, composable pipeline over chunks.
///
/// Transform methods consume `self` (move semantics) and return a new `LazyChunkStream`.
/// Work only happens when a terminal (`write_rrd`, `collect`, iterator) is called.
///
/// This type is intentionally **not** `Clone`. The PyO3 layer enforces move semantics via
/// `Option::take()`, preventing accidental reuse of a stream after it has been consumed by a
/// builder method. The only shared reference is `Arc<SplitOrigin>`, which gives both branches
/// of a split a shared identity so the engine can compile the upstream once.
pub struct LazyChunkStream {
    source: StreamSource,
    steps: Vec<PipelineStep>,
}

impl LazyChunkStream {
    /// Create a lazy stream backed by a factory (e.g. a reader).
    pub fn from_factory(factory: impl ChunkStreamFactory + 'static) -> Self {
        Self {
            source: StreamSource::StreamFactory(Box::new(factory)),
            steps: Vec::new(),
        }
    }

    pub fn from_py_iter(obj: Py<PyAny>) -> Self {
        Self {
            source: StreamSource::PyIterable(obj),
            steps: Vec::new(),
        }
    }

    pub fn filter(mut self, f: StructuredFilter) -> Self {
        self.steps.push(PipelineStep::Filter(f));
        self
    }

    pub fn drop_matching(mut self, f: StructuredFilter) -> Self {
        self.steps.push(PipelineStep::Drop(f));
        self
    }

    pub fn lenses(
        mut self,
        lenses: re_lenses_core::Lenses,
        content: Option<re_log_types::ResolvedEntityPathFilter>,
    ) -> Self {
        self.steps.push(PipelineStep::Lenses { lenses, content });
        self
    }

    pub fn map(mut self, callable: Py<PyAny>) -> Self {
        self.steps.push(PipelineStep::Map(callable));
        self
    }

    pub fn flat_map(mut self, callable: Py<PyAny>) -> Self {
        self.steps.push(PipelineStep::FlatMap(callable));
        self
    }

    pub fn split(self, f: StructuredFilter) -> (Self, Self) {
        let origin = Arc::new(SplitOrigin {
            upstream: self,
            filter: f,
        });

        let branch_match = Self {
            source: StreamSource::SplitBranch {
                origin: Arc::clone(&origin),
                side: SplitSide::Matched,
            },
            steps: Vec::new(),
        };

        let branch_nomatch = Self {
            source: StreamSource::SplitBranch {
                origin,
                side: SplitSide::Unmatched,
            },
            steps: Vec::new(),
        };

        (branch_match, branch_nomatch)
    }

    pub fn merge(streams: Vec<Self>) -> Self {
        Self {
            source: StreamSource::Merged(streams),
            steps: Vec::new(),
        }
    }

    // --- Accessors (used by engine::compile) ---

    pub fn source(&self) -> &StreamSource {
        &self.source
    }

    pub fn steps(&self) -> &[PipelineStep] {
        &self.steps
    }

    // --- Terminals (trigger execution via engine) ---

    /// Compile the lazy pipeline into a pull-based [`ChunkStream`].
    ///
    /// Each call produces a fully independent execution: fresh file handles, fresh channels
    /// for splits, fresh routing threads. There is no shared mutable state between compile
    /// calls — the same `LazyChunkStream` can be compiled and drained repeatedly.
    ///
    /// The one exception is **`PyIterator`** sources: Python iterators are inherently stateful,
    /// so compiling twice shares the same (potentially exhausted) iterator object. A proper
    /// solution would separate the pipeline *description* from its *inputs*, allowing the
    /// description to be compiled repeatedly with fresh bindings each time. That is deferred
    /// to a future PR.
    pub fn compile(&self) -> Box<dyn ChunkStream> {
        super::engine::compile(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_log_types::{EntityPathFilter, ResolvedEntityPathFilter};
    use re_types_core::{ComponentIdentifier, TimelineName};

    fn epf(s: &str) -> ResolvedEntityPathFilter {
        EntityPathFilter::parse_forgiving(s).resolve_without_substitutions()
    }

    fn comp(s: &str) -> ComponentIdentifier {
        ComponentIdentifier::new(s)
    }

    #[test]
    fn test_merge_disjoint_fields() {
        let a = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            ..Default::default()
        };
        let b = StructuredFilter {
            is_static: Some(true),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert!(m.content.is_some());
                assert_eq!(m.is_static, Some(true));
                assert!(m.has_timeline.is_none());
                assert!(m.components.is_none());
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_same_timeline() {
        let a = StructuredFilter {
            has_timeline: Some(TimelineName::from("frame")),
            ..Default::default()
        };
        let b = StructuredFilter {
            has_timeline: Some(TimelineName::from("frame")),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert_eq!(m.has_timeline, Some(TimelineName::from("frame")));
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_different_timeline() {
        let a = StructuredFilter {
            has_timeline: Some(TimelineName::from("frame")),
            ..Default::default()
        };
        let b = StructuredFilter {
            has_timeline: Some(TimelineName::from("log_time")),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Conflict => {}
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_is_static_conflict() {
        let a = StructuredFilter {
            is_static: Some(true),
            ..Default::default()
        };
        let b = StructuredFilter {
            is_static: Some(false),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Empty => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_content_conflict() {
        let a = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            ..Default::default()
        };
        let b = StructuredFilter {
            content: Some(epf("+ /camera/**")),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Conflict => {}
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_content_equal() {
        let a = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            ..Default::default()
        };
        let b = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert_eq!(m.content, Some(epf("+ /robot/**")));
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_components_intersect() {
        let a = StructuredFilter {
            components: Some(vec![comp("A"), comp("B")]),
            ..Default::default()
        };
        let b = StructuredFilter {
            components: Some(vec![comp("B"), comp("C")]),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert_eq!(m.components, Some(vec![comp("B")]));
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_components_disjoint() {
        let a = StructuredFilter {
            components: Some(vec![comp("A")]),
            ..Default::default()
        };
        let b = StructuredFilter {
            components: Some(vec![comp("B")]),
            ..Default::default()
        };
        match a.try_merge(&b) {
            MergeResult::Empty => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_components_one_none() {
        let a = StructuredFilter {
            components: Some(vec![comp("A"), comp("B")]),
            ..Default::default()
        };
        let b = StructuredFilter::default();
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert_eq!(m.components, Some(vec![comp("A"), comp("B")]));
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_all_none() {
        let a = StructuredFilter::default();
        let b = StructuredFilter::default();
        match a.try_merge(&b) {
            MergeResult::Merged(m) => {
                assert!(m.is_noop());
            }
            other => panic!("expected Merged, got {other:?}"),
        }
    }

    #[test]
    fn test_is_noop() {
        assert!(StructuredFilter::default().is_noop());
        assert!(
            !StructuredFilter {
                content: Some(epf("+ /a/**")),
                ..Default::default()
            }
            .is_noop()
        );
        assert!(
            !StructuredFilter {
                has_timeline: Some(TimelineName::from("frame")),
                ..Default::default()
            }
            .is_noop()
        );
        assert!(
            !StructuredFilter {
                is_static: Some(false),
                ..Default::default()
            }
            .is_noop()
        );
        assert!(
            !StructuredFilter {
                components: Some(vec![comp("A")]),
                ..Default::default()
            }
            .is_noop()
        );
    }
}
