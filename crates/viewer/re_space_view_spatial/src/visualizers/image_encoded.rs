use itertools::Itertools as _;

use re_query::range_zip_1x2;
use re_space_view::HybridResults;
use re_types::{
    archetypes::ImageEncoded,
    components::{Blob, DrawOrder, MediaType, Opacity},
    tensor_data::TensorDataMeaning,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ImageDecodeCache, QueryContext, SpaceViewClass,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_2d_entities, textured_rect_from_image},
    PickableImageRect, SpatialSpaceView2D,
};

use super::{
    bounding_box_for_textured_rect, entity_iterator::process_archetype, SpatialViewVisualizerData,
};

pub struct ImageEncodedVisualizer {
    pub data: SpatialViewVisualizerData,
    pub images: Vec<PickableImageRect>,
}

impl Default for ImageEncodedVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
        }
    }
}

impl IdentifiedViewSystem for ImageEncodedVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "ImageEncoded".into()
    }
}

impl VisualizerSystem for ImageEncodedVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<ImageEncoded>()
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

        process_archetype::<Self, ImageEncoded, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| self.process_image_encoded(ctx, results, spatial_ctx),
        )?;

        // TODO(#702): draw order is translated to depth offset, which works fine for opaque images,
        // but for everything with transparency, actual drawing order is still important.
        // We mitigate this a bit by at least sorting the images within each other.
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

impl ImageEncodedVisualizer {
    fn process_image_encoded(
        &mut self,
        ctx: &QueryContext<'_>,
        results: &HybridResults<'_>,
        spatial_ctx: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        use re_space_view::RangeResultsExt as _;

        let resolver = ctx.recording().resolver();
        let entity_path = ctx.target_entity_path;

        let blobs = match results.get_required_component_dense::<Blob>(resolver) {
            Some(blobs) => blobs?,
            _ => return Ok(()),
        };

        let media_types = results.get_or_empty_dense::<MediaType>(resolver)?;
        let opacities = results.get_or_empty_dense::<Opacity>(resolver)?;

        for (&(_time, tensor_data_row_id), blobs, media_types, opacities) in range_zip_1x2(
            blobs.range_indexed(),
            media_types.range_indexed(),
            opacities.range_indexed(),
        ) {
            let Some(blob) = blobs.first() else {
                continue;
            };
            let media_type = media_types.and_then(|media_types| media_types.first());

            let image = ctx.viewer_ctx.cache.entry(|c: &mut ImageDecodeCache| {
                c.entry(tensor_data_row_id, blob, media_type.map(|mt| mt.as_str()))
            });

            let image = match image {
                Ok(image) => image,
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to decode ImageEncoded at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            let opacity = opacities.and_then(|opacity| opacity.first());

            let opacity = opacity.copied().unwrap_or_else(|| self.fallback_for(ctx));
            let multiplicative_tint =
                re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));

            if let Some(textured_rect) = textured_rect_from_image(
                ctx.viewer_ctx,
                entity_path,
                spatial_ctx,
                &image,
                multiplicative_tint,
            ) {
                // Only update the bounding box if this is a 2D space view.
                // This is avoids a cyclic relationship where the image plane grows
                // the bounds which in turn influence the size of the image plane.
                // See: https://github.com/rerun-io/rerun/issues/3728
                if spatial_ctx.space_view_class_identifier == SpatialSpaceView2D::identifier() {
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
                    meaning: TensorDataMeaning::Unknown,
                    depth_meter: None,
                    tensor: None,
                    image: Some(image),
                });
            }
        }

        Ok(())
    }
}

impl TypedComponentFallbackProvider<Opacity> for ImageEncodedVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Opacity {
        1.0.into()
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for ImageEncodedVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_IMAGE
    }
}

re_viewer_context::impl_component_fallback_provider!(ImageEncodedVisualizer => [DrawOrder, Opacity]);
