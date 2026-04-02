//! Pull-based execution engine for chunk pipelines.
//!
//! This module contains the runtime machinery that powers [`super::stream::LazyChunkStream`].
//! Everything here sits behind the [`super::stream::LazyChunkStream::compile`] boundary: the
//! pipeline description builds up a declarative graph, and `compile()` turns it into a chain of
//! [`ChunkStream`] implementors defined here.
//!
//! Each `compile()` call creates a [`CompileContext`] that carries whole-graph state for that
//! single compilation pass (e.g. shared split channels). There is no mutable state shared across
//! compile calls — the same pipeline description can be compiled and drained repeatedly.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;

use re_chunk::Chunk;

use super::ChunkStream;
use super::error::{ChunkPipelineError, PythonException};
use super::stream::{
    LazyChunkStream, PipelineStep, SplitOrigin, SplitSide, StreamSource, StructuredFilter,
};

/// Compile a [`LazyChunkStream`] into a runnable [`ChunkStream`] chain.
pub fn compile(stream: &LazyChunkStream) -> Box<dyn ChunkStream> {
    // First pass: detect dangling split branches so we can degenerate them to filter/drop
    // (a dangling branch would otherwise stall the pipeline via its bounded channel).
    let split_usage = scan_split_usage(stream);

    // Second pass: turn the DAG into a runnable stream.
    let mut ctx = CompileContext::new(split_usage);
    compile_inner(stream, &mut ctx)
}

// ---------------------------------------------------------------------------
// Compile
// ---------------------------------------------------------------------------

/// Channel payload: either a chunk or an error from the upstream thread.
type ChannelItem = Result<Arc<Chunk>, ChunkPipelineError>;

/// Newtype adapter to use `Arc<SplitOrigin>` as a map key, using pointer identity (which is the
/// convention used by [`LazyChunkStream`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SplitOriginId(usize);

impl SplitOriginId {
    fn new(split_origin: &Arc<SplitOrigin>) -> Self {
        Self(Arc::as_ptr(split_origin) as usize)
    }
}

/// The two outbound channels of a split node.
struct SplitChannels {
    rx_match: Option<crossbeam::channel::Receiver<ChannelItem>>,
    rx_nomatch: Option<crossbeam::channel::Receiver<ChannelItem>>,
}

/// Which sides of a split are reachable from the sink.
#[derive(Default)]
struct SplitUsage {
    matched_used: bool,
    unmatched_used: bool,

    /// Prevents scanning the same split's upstream twice.
    upstream_scanned: bool,
}

/// Per-compilation-pass state.
///
/// Carries whole-graph information that is needed when compiling a pipeline tree into a
/// [`ChunkStream`] chain. Currently this is the split channel registry and the reachability
/// information from the scan pass.
struct CompileContext {
    /// For each [`SplitOrigin`], the pair of receivers produced by the routing thread.
    /// The first branch to compile a given split spawns the thread and stashes both
    /// receivers here; the second branch takes the remaining one.
    split_channels: HashMap<SplitOriginId, SplitChannels>,

    /// Per-split reachability from the scan pass.
    split_usage: HashMap<SplitOriginId, SplitUsage>,
}

impl CompileContext {
    fn new(split_usage: HashMap<SplitOriginId, SplitUsage>) -> Self {
        Self {
            split_channels: HashMap::new(),
            split_usage,
        }
    }

    /// Get or create the channel pair for a split. On first call for a given `desc`,
    /// compiles the upstream, spawns the routing thread, and returns the requested receiver.
    /// On second call, returns the remaining receiver.
    fn take_split_receiver(
        &mut self,
        desc: &Arc<SplitOrigin>,
        side: SplitSide,
    ) -> crossbeam::channel::Receiver<ChannelItem> {
        let key = SplitOriginId::new(desc);

        if !self.split_channels.contains_key(&key) {
            // Compile the shared upstream within this same context. We do this before
            // inserting into the map to avoid a borrow conflict. This is safe because
            // the upstream cannot contain this split (that would be a cycle).
            let upstream = compile_inner(&desc.upstream, self);

            let (tx_match, rx_match) =
                crossbeam::channel::bounded::<ChannelItem>(super::CHUNK_CHANNEL_CAPACITY);
            let (tx_nomatch, rx_nomatch) =
                crossbeam::channel::bounded::<ChannelItem>(super::CHUNK_CHANNEL_CAPACITY);

            let filter = desc.filter.clone();

            std::thread::Builder::new()
                .name("chunk-split-router".into())
                .spawn(move || {
                    let mut stream = upstream;
                    let mut match_alive = true;
                    let mut nomatch_alive = true;

                    loop {
                        match stream.next() {
                            Ok(Some(chunk)) => {
                                let (matching, complement) = filter.split(chunk);

                                if match_alive
                                    && let Some(m) = matching
                                    && re_quota_channel::send_crossbeam(&tx_match, Ok(m)).is_err()
                                {
                                    match_alive = false;
                                }

                                if nomatch_alive
                                    && let Some(c) = complement
                                    && re_quota_channel::send_crossbeam(&tx_nomatch, Ok(c)).is_err()
                                {
                                    nomatch_alive = false;
                                }

                                if !match_alive && !nomatch_alive {
                                    break;
                                }
                            }

                            Ok(None) => break,

                            Err(err) => {
                                // Send the error to both live branches so the sink
                                // sees it regardless of which branch it reads first.
                                if match_alive {
                                    re_quota_channel::send_crossbeam(&tx_match, Err(err.clone()))
                                        .ok();
                                }

                                if nomatch_alive {
                                    re_quota_channel::send_crossbeam(&tx_nomatch, Err(err)).ok();
                                }

                                break;
                            }
                        }
                    }
                })
                .expect("Failed to spawn split router thread");

            self.split_channels.insert(
                key,
                SplitChannels {
                    rx_match: Some(rx_match),
                    rx_nomatch: Some(rx_nomatch),
                },
            );
        }

        let channels = self.split_channels.get_mut(&key).expect("just inserted");

        match side {
            SplitSide::Matched => channels.rx_match.take().expect(
                "split matched-branch receiver already taken (compiled twice in same pass?)",
            ),
            SplitSide::Unmatched => channels.rx_nomatch.take().expect(
                "split unmatched-branch receiver already taken (compiled twice in same pass?)",
            ),
        }
    }

    /// Returns `true` if both sides of the split are reachable from the sink.
    fn split_has_both_sides(&self, key: &SplitOriginId) -> bool {
        self.split_usage
            .get(key)
            .is_some_and(|u| u.matched_used && u.unmatched_used)
    }
}

// ---------------------------------------------------------------------------
// Scan pass — detect split branch reachability
// ---------------------------------------------------------------------------

fn scan_split_usage(root: &LazyChunkStream) -> HashMap<SplitOriginId, SplitUsage> {
    let mut usage = HashMap::new();
    scan_inner(root, &mut usage);
    usage
}

fn scan_inner(stream: &LazyChunkStream, usage: &mut HashMap<SplitOriginId, SplitUsage>) {
    match stream.source() {
        StreamSource::SplitBranch { origin, side } => {
            let key = SplitOriginId::new(origin);
            let entry = usage.entry(key).or_default();
            match side {
                SplitSide::Matched => entry.matched_used = true,
                SplitSide::Unmatched => entry.unmatched_used = true,
            }
            if !entry.upstream_scanned {
                entry.upstream_scanned = true;
                scan_inner(&origin.upstream, usage);
            }
        }

        StreamSource::Merged(streams) => {
            for s in streams {
                scan_inner(s, usage);
            }
        }

        StreamSource::StreamFactory(_) | StreamSource::PyIterable(_) => {}
    }
}

/// Recursive compilation with a shared context.
fn compile_inner(stream: &LazyChunkStream, ctx: &mut CompileContext) -> Box<dyn ChunkStream> {
    let mut compiled: Box<dyn ChunkStream> = match stream.source() {
        StreamSource::StreamFactory(factory) => match factory.create() {
            Ok(source) => source,
            Err(err) => Box::new(FailedSource(Some(err))),
        },

        StreamSource::PyIterable(obj) => {
            let cloned = Python::attach(|py| obj.clone_ref(py));
            Box::new(PyIteratorSource::new(cloned))
        }

        StreamSource::Merged(streams) => {
            let compiled: Vec<Box<dyn ChunkStream>> =
                streams.iter().map(|s| compile_inner(s, ctx)).collect();

            let (tx, rx) =
                crossbeam::channel::bounded::<ChannelItem>(super::CHUNK_CHANNEL_CAPACITY);
            for upstream in compiled {
                let tx = tx.clone();
                std::thread::Builder::new()
                    .name("chunk-merge-source".into())
                    .spawn(move || {
                        let mut stream = upstream;
                        loop {
                            match stream.next() {
                                Ok(Some(chunk)) => {
                                    if re_quota_channel::send_crossbeam(&tx, Ok(chunk)).is_err() {
                                        break; // receiver dropped
                                    }
                                }

                                Ok(None) => break,

                                Err(err) => {
                                    re_quota_channel::send_crossbeam(&tx, Err(err)).ok();
                                    break;
                                }
                            }
                        }
                    })
                    .expect("Failed to spawn merge source thread");
            }
            drop(tx); // channel closes when all source threads finish
            Box::new(ChannelStream::new(rx))
        }

        StreamSource::SplitBranch { origin, side } => {
            let key = SplitOriginId::new(origin);
            if ctx.split_has_both_sides(&key) {
                // Both branches are reachable: use the full router thread.
                let rx = ctx.take_split_receiver(origin, *side);
                Box::new(ChannelStream::new(rx))
            } else {
                // Only one branch is reachable: degenerate to filter/drop.
                re_log::warn!(
                    "Only one branch of a split is connected to the pipeline. \
                     The split has been optimized into a filter/drop operation."
                );
                let upstream = compile_inner(&origin.upstream, ctx);
                match side {
                    SplitSide::Matched => {
                        Box::new(FilterStream::new(upstream, origin.filter.clone()))
                    }
                    SplitSide::Unmatched => {
                        Box::new(DropStream::new(upstream, origin.filter.clone()))
                    }
                }
            }
        }
    };

    for step in stream.steps() {
        compiled = match step {
            PipelineStep::Filter(f) => Box::new(FilterStream::new(compiled, f.clone())),
            PipelineStep::Drop(f) => Box::new(DropStream::new(compiled, f.clone())),
        };
    }

    compiled
}

// ---------------------------------------------------------------------------
// PyIteratorSource
// ---------------------------------------------------------------------------

pub struct PyIteratorSource {
    iter_obj: Py<PyAny>,
}

impl PyIteratorSource {
    pub fn new(iterable: Py<PyAny>) -> Self {
        Self { iter_obj: iterable }
    }
}

impl ChunkStream for PyIteratorSource {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        Python::attach(|py| {
            let iter = self.iter_obj.bind(py);
            match iter.call_method0("__next__") {
                Ok(obj) => {
                    let internal: PyRef<'_, crate::chunk::PyChunkInternal> =
                        obj.extract().map_err(|err| {
                            ChunkPipelineError::PythonIterator(PythonException::new(err))
                        })?;
                    Ok(Some(Arc::clone(internal.inner())))
                }

                Err(err) if err.is_instance_of::<PyStopIteration>(py) => Ok(None),

                Err(err) => Err(ChunkPipelineError::PythonIterator(PythonException::new(
                    err,
                ))),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// FilterStream
// ---------------------------------------------------------------------------

pub struct FilterStream {
    inner: Box<dyn ChunkStream>,
    filter: StructuredFilter,
}

impl FilterStream {
    pub fn new(inner: Box<dyn ChunkStream>, filter: StructuredFilter) -> Self {
        Self { inner, filter }
    }
}

impl ChunkStream for FilterStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        loop {
            let Some(chunk) = self.inner.next()? else {
                return Ok(None);
            };

            if let Some(filtered) = self.filter.apply(chunk) {
                return Ok(Some(filtered));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DropStream
// ---------------------------------------------------------------------------

pub struct DropStream {
    inner: Box<dyn ChunkStream>,
    filter: StructuredFilter,
}

impl DropStream {
    pub fn new(inner: Box<dyn ChunkStream>, filter: StructuredFilter) -> Self {
        Self { inner, filter }
    }
}

impl ChunkStream for DropStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        loop {
            let Some(chunk) = self.inner.next()? else {
                return Ok(None);
            };

            if let Some(complement) = self.filter.apply_complement(chunk) {
                return Ok(Some(complement));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelStream
// ---------------------------------------------------------------------------

pub struct ChannelStream {
    rx: crossbeam::channel::Receiver<ChannelItem>,
}

impl ChannelStream {
    pub fn new(rx: crossbeam::channel::Receiver<ChannelItem>) -> Self {
        Self { rx }
    }
}

impl ChunkStream for ChannelStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(_) => Ok(None), // channel closed — stream exhausted
        }
    }
}

// ---------------------------------------------------------------------------
// FailedSource
// ---------------------------------------------------------------------------

/// A stream that yields a single error, then terminates.
/// Used to defer factory creation errors to the pipeline consumer.
struct FailedSource(Option<ChunkPipelineError>);

impl ChunkStream for FailedSource {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.0.take() {
            Some(err) => Err(err),
            None => Ok(None),
        }
    }
}
