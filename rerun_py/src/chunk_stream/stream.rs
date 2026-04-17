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

impl StructuredFilter {
    /// Check chunk-level predicates (content, has_timeline, is_static).
    /// Returns `true` if all pass.
    fn predicates_match(&self, chunk: &Chunk) -> bool {
        if let Some(ref content) = self.content
            && !content.matches(chunk.entity_path())
        {
            return false;
        }
        if let Some(ref timeline) = self.has_timeline
            && !chunk.timelines().contains_key(timeline)
        {
            return false;
        }
        if let Some(is_static) = self.is_static
            && chunk.is_static() != is_static
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
        if !self.predicates_match(&chunk) {
            return None;
        }

        if let Some(ref components) = self.components {
            // OR semantics: chunk must have at least one of the listed components.
            let has_any = components
                .iter()
                .any(|c| chunk.components().contains_component(*c));
            if !has_any {
                return None;
            }
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
        if !self.predicates_match(&chunk) {
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
        if !self.predicates_match(&chunk) {
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
    Lenses(re_lenses_core::Lenses),
    Map(Py<PyAny>),
    FlatMap(Py<PyAny>),
}

impl Clone for PipelineStep {
    fn clone(&self) -> Self {
        match self {
            Self::Filter(f) => Self::Filter(f.clone()),
            Self::Drop(f) => Self::Drop(f.clone()),
            Self::Lenses(l) => Self::Lenses(l.clone()),
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

    pub fn lenses(mut self, lenses: re_lenses_core::Lenses) -> Self {
        self.steps.push(PipelineStep::Lenses(lenses));
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
