use std::borrow::Cow;

use re_chunk_store::external::re_chunk::{ChunkComponentIter, ChunkComponentSlicer};
use re_chunk_store::{Chunk, Span};
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_sdk_types::{Component, ComponentIdentifier, RowId};

/// A helper struct that bundles a list of chunks with a component identifier.
///
/// This is useful when looking up chunks that contain a specific component:
/// Since the referenced chunks may contain multiple components,
/// subsequent lookups for data inside those chunks need the component identifier again.
/// By bundling references to chunks and component identifier,
/// we can avoid having to pass the identifier around in the code.
#[derive(Debug, Clone)]
pub struct ChunksWithComponent<'chunk> {
    pub chunks: Cow<'chunk, [Chunk]>,
    pub component: ComponentIdentifier,
}

impl ChunksWithComponent<'_> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = ChunkWithComponent<'_>> {
        self.chunks.iter().map(move |chunk| ChunkWithComponent {
            chunk,
            component: self.component,
        })
    }

    #[inline]
    pub fn empty(component: ComponentIdentifier) -> Self {
        Self {
            chunks: Cow::Borrowed(&[]),
            component,
        }
    }
}

/// Like [`ChunksWithComponent`] but for a single chunk.
#[derive(Debug, Clone, Copy)]
pub struct ChunkWithComponent<'chunk> {
    pub chunk: &'chunk Chunk,
    pub component: ComponentIdentifier,
}

impl<'chunk> ChunkWithComponent<'chunk> {
    /// See [`Chunk::iter_component_indices`].
    #[inline]
    pub fn iter_component_indices(
        &self,
        timeline: TimelineName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + 'chunk + use<'chunk> {
        self.chunk.iter_component_indices(timeline, self.component)
    }

    /// See [`Chunk::iter_slices`].
    #[inline]
    pub fn iter_slices<S: ChunkComponentSlicer + 'chunk>(
        &self,
    ) -> impl Iterator<Item = S::Item<'chunk>> + 'chunk + use<'chunk, S> {
        self.chunk.iter_slices::<S>(self.component)
    }

    /// See [`Chunk::iter_component`].
    #[inline]
    pub fn iter_component<C: Component>(
        &self,
    ) -> ChunkComponentIter<C, impl Iterator<Item = Span<usize>> + 'chunk + use<'chunk, C>> {
        self.chunk.iter_component::<C>(self.component)
    }

    /// See [`Chunk::iter_component_timepoints`].
    #[inline]
    pub fn iter_component_timepoints(
        &self,
    ) -> impl Iterator<Item = TimePoint> + 'chunk + use<'chunk> {
        self.chunk.iter_component_timepoints(self.component)
    }
}
