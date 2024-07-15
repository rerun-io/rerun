use itertools::Itertools as _;

use re_chunk_store::RowId;
use re_log_types::TimeInt;
use re_query::range_zip_1x1;
use re_space_view::diff_component_filter;
use re_types::{
    archetypes::SegmentationImage,
    components::{DrawOrder, Opacity, TensorData},
    tensor_data::TensorDataMeaning,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, QueryContext, SpaceViewClass,
    SpaceViewSystemExecutionError, TensorDecodeCache, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerAdditionalApplicabilityFilter, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    ui::SpatialSpaceViewState, view_kind::SpatialSpaceViewKind,
    visualizers::filter_visualizable_2d_entities, PickableImageRect, SpatialSpaceView2D,
};

use super::{bounding_box_for_textured_rect, tensor_to_textured_rect, SpatialViewVisualizerData};

pub struct SegmentationImageVisualizer {
    pub data: SpatialViewVisualizerData,
    pub images: Vec<PickableImageRect>,
}

impl Default for SegmentationImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
        }
    }
}

struct SegmentationImageComponentData<'a> {
    index: (TimeInt, RowId),

    tensor: &'a TensorData,
    opacity: Option<&'a Opacity>,
}

impl IdentifiedViewSystem for SegmentationImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SegmentationImage".into()
    }
}

struct SegmentationImageVisualizerEntityFilter;

impl VisualizerAdditionalApplicabilityFilter for SegmentationImageVisualizerEntityFilter {
    fn update_applicability(&mut self, event: &re_chunk_store::ChunkStoreEvent) -> bool {
        diff_component_filter(event, |tensor: &re_types::components::TensorData| {
            tensor.is_shaped_like_an_image()
        })
    }
}

impl VisualizerSystem for SegmentationImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<SegmentationImage>()
    }

    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        Some(Box::new(SegmentationImageVisualizerEntityFilter))
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_2d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        super::entity_iterator::process_archetype::<Self, SegmentationImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let entity_path = ctx.target_entity_path;
                let resolver = ctx.recording().resolver();

                let tensors = match results.get_required_component_dense::<TensorData>(resolver) {
                    Some(tensors) => tensors?,
                    _ => return Ok(()),
                };

                let opacity = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x1(tensors.range_indexed(), opacity.range_indexed())
                    .filter_map(|(&index, tensors, opacity)| {
                        tensors
                            .first()
                            .map(|tensor| SegmentationImageComponentData {
                                index,
                                tensor,
                                opacity: opacity.and_then(|opacity| opacity.first()),
                            })
                    });

                let meaning = TensorDataMeaning::ClassId;

                for data in data {
                    if !data.tensor.is_shaped_like_an_image() {
                        continue;
                    }

                    let tensor_data_row_id = data.index.1;
                    let tensor = match ctx.viewer_ctx.cache.entry(|c: &mut TensorDecodeCache| {
                        c.entry(tensor_data_row_id, data.tensor.0.clone())
                    }) {
                        Ok(tensor) => tensor,
                        Err(err) => {
                            re_log::warn_once!(
                                "Encountered problem decoding tensor at path {entity_path}: {err}"
                            );
                            continue;
                        }
                    };

                    // TODO(andreas): colormap is only available for depth images right now.
                    let colormap = None;

                    let opacity = data
                        .opacity
                        .copied()
                        .unwrap_or_else(|| self.fallback_for(ctx));
                    let multiplicative_tint =
                        re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));

                    if let Some(textured_rect) = tensor_to_textured_rect(
                        ctx.viewer_ctx,
                        entity_path,
                        spatial_ctx,
                        tensor_data_row_id,
                        &tensor,
                        meaning,
                        multiplicative_tint,
                        colormap,
                    ) {
                        // Only update the bounding box if this is a 2D space view.
                        // This is avoids a cyclic relationship where the image plane grows
                        // the bounds which in turn influence the size of the image plane.
                        // See: https://github.com/rerun-io/rerun/issues/3728
                        if spatial_ctx.space_view_class_identifier
                            == SpatialSpaceView2D::identifier()
                        {
                            self.data.add_bounding_box(
                                entity_path.hash(),
                                bounding_box_for_textured_rect(&textured_rect),
                                spatial_ctx.world_from_entity,
                            );
                        }

                        self.images.push(PickableImageRect {
                            ent_path: entity_path.clone(),
                            row_id: tensor_data_row_id,
                            textured_rect,
                            tensor,
                        });
                    }
                }

                Ok(())
            },
        )?;

        // TODO(#702): draw order is translated to depth offset, which works fine for opaque images,
        // but for everything with transparency, actual drawing order is still important.
        // We mitigate this a bit by at least sorting the segmentation images within each other.
        // Sorting of Images vs DepthImage vs SegmentationImage uses the fact that
        // visualizers are executed in the order of their identifiers.
        // -> The draw order is always DepthImage then Image then SegmentationImage,
        //    which happens to be exactly what we want 🙈
        self.images.sort_by_key(|image| {
            (
                image.textured_rect.options.depth_offset,
                egui::emath::OrderedFloat(image.textured_rect.options.multiplicative_tint.a()),
            )
        });

        let mut draw_data_list = Vec::new();

        // TODO(wumpf): Can we avoid this copy, maybe let DrawData take an iterator?
        let rectangles = self
            .images
            .iter()
            .map(|image| image.textured_rect.clone())
            .collect_vec();
        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!("Failed to create rectangle draw data from images: {err}");
            }
        }

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Opacity> for SegmentationImageVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Opacity {
        // Segmentation images should be transparent whenever they're on top of other images,
        // But fully opaque if there are no other images in the scene.
        let Some(view_state) = ctx
            .view_state
            .as_any()
            .downcast_ref::<SpatialSpaceViewState>()
        else {
            return 1.0.into();
        };

        // Known cosmetic issues with this approach:
        // * The first frame we have more than one image, the segmentation image will be opaque.
        //      It's too complex to do a full view query just for this here.
        //      However, we should be able to analyze the `DataQueryResults` instead to check how many entities are fed to the Image/DepthImage visualizers.
        // * In 3D scenes, images that are on a completely different plane will cause this to become transparent.
        if view_state.num_non_segmentation_images_last_frame == 0 {
            1.0
        } else {
            0.5
        }
        .into()
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for SegmentationImageVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_SEGMENTATION_IMAGE
    }
}

re_viewer_context::impl_component_fallback_provider!(SegmentationImageVisualizer => [DrawOrder, Opacity]);
