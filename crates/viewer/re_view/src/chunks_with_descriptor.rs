use std::borrow::Cow;

use re_chunk_store::{
    external::re_chunk::{ChunkComponentIter, ChunkComponentSlicer},
    Chunk,
};
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_types::{Component, ComponentDescriptor, RowId};

/// A helper struct that bundles a list of chunks with a component descriptor.
///
/// This is useful when looking up chunks that contain a specific component descriptor:
/// Since the referenced chunks may contain multiple components,
/// subsequent lookups for data inside those chunks need the component descriptor again.
/// By bundling references to chunks and descriptor,
/// we can avoid having to pass the descriptor around in the code.
#[derive(Debug, Clone)]
pub struct ChunksWithDescriptor<'chunk> {
    pub chunks: Cow<'chunk, [Chunk]>,
    pub component_descriptor: ComponentDescriptor,
}

impl ChunksWithDescriptor<'_> {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = ChunkWithDescriptor<'_, '_>> {
        self.chunks.iter().map(move |chunk| ChunkWithDescriptor {
            chunk,
            descriptor: &self.component_descriptor,
        })
    }
}

/// Like [`ChunksWithDescriptor`] but for a single chunk.
///
/// Note that the descriptor is not owned, but borrowed here since it's typically returned
/// only by iterating over a [`ChunksWithDescriptor`].
// TODO(#9903): Chunk descriptor referencing should be made trivial so that it doesn't matter whether we borrow or copy it.
#[derive(Debug, Clone)]
pub struct ChunkWithDescriptor<'chunk, 'descriptor> {
    pub chunk: &'chunk Chunk,
    pub descriptor: &'descriptor ComponentDescriptor,
}

impl<'chunk> ChunkWithDescriptor<'chunk, '_> {
    /// See [`Chunk::iter_component_indices`].
    #[inline]
    pub fn iter_component_indices(
        &self,
        timeline: &TimelineName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + 'chunk + use<'chunk> {
        self.chunk.iter_component_indices(timeline, self.descriptor)
    }

    /// See [`Chunk::iter_slices`].
    #[inline]
    pub fn iter_slices<S: ChunkComponentSlicer + 'chunk>(
        &self,
    ) -> impl Iterator<Item = S::Item<'chunk>> + 'chunk + use<'chunk, S> {
        self.chunk.iter_slices::<S>(self.descriptor.clone())
    }

    /// See [`Chunk::iter_component`].
    #[inline]
    pub fn iter_component<C: Component>(
        &self,
    ) -> ChunkComponentIter<C, impl Iterator<Item = (usize, usize)> + 'chunk + use<'chunk, C>> {
        self.chunk.iter_component::<C>(self.descriptor)
    }

    /// See [`Chunk::iter_component_timepoints`].
    #[inline]
    pub fn iter_component_timepoints(
        &self,
    ) -> impl Iterator<Item = TimePoint> + 'chunk + use<'chunk> {
        self.chunk.iter_component_timepoints(self.descriptor)
    }
}
