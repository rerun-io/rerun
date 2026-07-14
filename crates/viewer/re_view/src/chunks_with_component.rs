use std::borrow::Cow;

use itertools::Either;
use re_chunk_store::external::re_chunk::{ChunkComponentIter, ChunkComponentSlicer};
use re_chunk_store::{Chunk, Span};
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_sdk_types::{Component, ComponentIdentifier, RowId};

use crate::ComponentMappingError;

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

/// Represents the result of trying to resolve a component to chunks while respecting blueprints.
///
/// With visualizer instructions, it can happen that resolving data with blueprint information fails,
/// for example due to errors in parsing the selector. This is codified in this struct and forces the
/// visualizer to handle the errors.
#[derive(Debug, Clone)]
pub struct MaybeChunksWithComponent<'chunk> {
    pub maybe_chunks: Result<Cow<'chunk, [Chunk]>, ComponentMappingError>,
    pub component: ComponentIdentifier,
}

impl MaybeChunksWithComponent<'_> {
    /// Iterates over chunks, or reports an error if chunk resolution failed.
    ///
    /// If the chunks were successfully resolved, returns an iterator over them.
    /// If there was an error during resolution, calls the `reporter` callback with the error
    /// and returns an empty iterator.
    ///
    /// The return type is `Either` to avoid boxing while still returning different iterator types.
    #[inline]
    pub fn iter(
        &self,
        mut reporter: impl FnMut(&ComponentMappingError),
    ) -> Either<
        // NOLINT
        impl Iterator<Item = ChunkWithComponent<'_>>,
        impl Iterator<Item = ChunkWithComponent<'_>>,
    > {
        match self.maybe_chunks.as_ref() {
            Ok(chunks) => Either::Left(chunks.iter().map(move |chunk| ChunkWithComponent {
                chunk,
                component: self.component,
            })),
            Err(err) => {
                reporter(err);
                Either::Right(std::iter::empty())
            }
        }
    }

    /// Creates a new instance with no chunks (successful but empty result).
    #[inline]
    pub fn empty(component: ComponentIdentifier) -> Self {
        Self {
            maybe_chunks: Ok(Cow::Borrowed(&[])),
            component,
        }
    }

    /// Creates a new instance representing a failure to resolve chunks.
    #[inline]
    pub fn error(component: ComponentIdentifier, err: ComponentMappingError) -> Self {
        Self {
            maybe_chunks: Err(err),
            component,
        }
    }
}

impl<'a> TryFrom<MaybeChunksWithComponent<'a>> for ChunksWithComponent<'a> {
    type Error = ComponentMappingError;

    #[inline]
    fn try_from(value: MaybeChunksWithComponent<'a>) -> Result<Self, Self::Error> {
        Ok(ChunksWithComponent {
            chunks: value.maybe_chunks?,
            component: value.component,
        })
    }
}

impl<'a> From<ChunksWithComponent<'a>> for MaybeChunksWithComponent<'a> {
    #[inline]
    fn from(ChunksWithComponent { chunks, component }: ChunksWithComponent<'a>) -> Self {
        Self {
            maybe_chunks: Ok(chunks),
            component,
        }
    }
}
