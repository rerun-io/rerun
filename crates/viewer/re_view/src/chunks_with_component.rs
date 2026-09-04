use std::borrow::Cow;

use itertools::Either;
use re_chunk_store::external::re_chunk::{ChunkComponentIter, ChunkComponentSlicer};
use re_chunk_store::{Chunk, Span};
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_sdk_types::{Component, ComponentIdentifier, RowId};
use re_viewer_context::{QueryContext, VisualizerComponentSource};

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

    /// Turns an empty successful result into an error when its required recording component has
    /// never been logged on the target entity.
    ///
    /// Existing data and errors pass through unchanged.
    /// Validation is also skipped while virtual chunks are pending and for blueprint-backed
    /// sources, so the entity schema is only queried when absence can be reported conclusively.
    pub fn ensure_required_component_present(
        self,
        has_pending_virtual_chunks: bool,
        query_context: &QueryContext<'_>,
        explicit_mapping: Option<&VisualizerComponentSource>,
    ) -> Self {
        if has_pending_virtual_chunks
            || !matches!(&self.maybe_chunks, Ok(chunks) if chunks.is_empty())
        {
            return self;
        }

        let source_component = match explicit_mapping {
            Some(VisualizerComponentSource::SourceComponent {
                source_component, ..
            }) => *source_component,

            Some(VisualizerComponentSource::Override | VisualizerComponentSource::Default) => {
                return self;
            }

            None => self.component,
        };

        let engine = query_context.view_ctx.recording_engine();
        let available_components = engine
            .schema()
            .all_components_for_entity(query_context.target_entity_path);
        if available_components.is_some_and(|components| components.contains(&source_component)) {
            return self;
        }

        Self::error(
            self.component,
            ComponentMappingError::component_not_present_on_entity(
                source_component,
                available_components.into_iter().flatten().copied(),
            ),
        )
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

#[cfg(test)]
mod tests {
    use re_log_types::{EntityPath, TimePoint};
    use re_sdk_types::{ComponentIdentifier, archetypes::Scalars};
    use re_test_context::TestContext;
    use re_types_core::ViewClassIdentifier;
    use re_viewer_context::{
        DataQueryResult, DataResult, QueryRange, ViewContext, ViewId, VisualizerComponentSource,
    };

    use super::MaybeChunksWithComponent;
    use crate::ComponentMappingError;

    #[test]
    fn required_component_presence_matrix() {
        let entity_path = EntityPath::from("entity");
        let mut test_context = TestContext::new();
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &Scalars::new([1.0]))
        });

        test_context.run(&egui::Context::default(), |viewer_ctx| {
            let data_result = DataResult {
                entity_path,
                any_visualizers_available: true,
                visualizer_instructions: Vec::new(),
                tree_prefix_only: false,
                visible: true,
                interactive: true,
                override_base_path: EntityPath::from("override"),
                query_range: QueryRange::LatestAt,
            };
            let view_ctx = ViewContext {
                viewer_ctx,
                view_id: ViewId::invalid(),
                view_class_identifier: ViewClassIdentifier::from_static_str("Test"),
                space_origin: &EntityPath::root(),
                view_state: &(),
                query_result: &DataQueryResult::default(),
            };
            let query_context =
                view_ctx.query_context(&data_result, view_ctx.current_query(), None);
            let target = "target".into();
            let missing_source = "source".into();
            let available_source = Scalars::descriptor_scalars().component;

            struct TestCase {
                name: &'static str,
                has_pending_virtual_chunks: bool,
                mapping: Option<VisualizerComponentSource>,
                expected_missing_component: Option<ComponentIdentifier>,
            }

            let cases = [
                TestCase {
                    name: "implicit missing source",
                    has_pending_virtual_chunks: false,
                    mapping: None,
                    expected_missing_component: Some(target),
                },
                TestCase {
                    name: "explicit missing source",
                    has_pending_virtual_chunks: false,
                    mapping: Some(VisualizerComponentSource::SourceComponent {
                        source_component: missing_source,
                        selector: String::new(),
                    }),
                    expected_missing_component: Some(missing_source),
                },
                TestCase {
                    name: "available source",
                    has_pending_virtual_chunks: false,
                    mapping: Some(VisualizerComponentSource::SourceComponent {
                        source_component: available_source,
                        selector: String::new(),
                    }),
                    expected_missing_component: None,
                },
                TestCase {
                    name: "blueprint override",
                    has_pending_virtual_chunks: false,
                    mapping: Some(VisualizerComponentSource::Override),
                    expected_missing_component: None,
                },
                TestCase {
                    name: "blueprint default",
                    has_pending_virtual_chunks: false,
                    mapping: Some(VisualizerComponentSource::Default),
                    expected_missing_component: None,
                },
                TestCase {
                    name: "pending virtual chunks",
                    has_pending_virtual_chunks: true,
                    mapping: None,
                    expected_missing_component: None,
                },
            ];

            for case in cases {
                let result = MaybeChunksWithComponent::empty(target)
                    .ensure_required_component_present(
                        case.has_pending_virtual_chunks,
                        &query_context,
                        case.mapping.as_ref(),
                    );
                let actual = result.maybe_chunks.err().map(|err| {
                    let ComponentMappingError::ComponentNotPresentOnEntity { component, .. } = err
                    else {
                        panic!("Expected a missing-component error");
                    };
                    component
                });
                assert_eq!(actual, case.expected_missing_component, "{}", case.name);
            }
        });
    }
}
