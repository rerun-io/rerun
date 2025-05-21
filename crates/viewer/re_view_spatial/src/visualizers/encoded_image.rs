use re_types::{
    archetypes::EncodedImage,
    components::{DrawOrder, MediaType, Opacity},
    image::ImageKind,
};
use re_view::HybridResults;
use re_viewer_context::{
    IdentifiedViewSystem, ImageDecodeCache, MaybeVisualizableEntities, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    PickableRectSourceData, PickableTexturedRect,
    contexts::SpatialSceneEntityContext,
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::{filter_visualizable_2d_entities, textured_rect_from_image},
};

use super::{SpatialViewVisualizerData, entity_iterator::process_archetype};

pub struct EncodedImageVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for EncodedImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

impl IdentifiedViewSystem for EncodedImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "EncodedImage".into()
    }
}

impl VisualizerSystem for EncodedImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<EncodedImage>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: MaybeVisualizableEntities,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        process_archetype::<Self, EncodedImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                self.process_encoded_image(ctx, results, spatial_ctx);
                Ok(())
            },
        )?;

        // TODO(#702): draw order is translated to depth offset, which works fine for opaque images,
        // but for everything with transparency, actual drawing order is still important.
        // We mitigate this a bit by at least sorting the images within each other.
        // Sorting of Images vs DepthImage vs SegmentationImage uses the fact that
        // visualizers are executed in the order of their identifiers.
        // -> The draw order is always DepthImage then Image then SegmentationImage,
        //    which happens to be exactly what we want 🙈
        self.data.pickable_rects.sort_by_key(|image| {
            (
                image.textured_rect.options.depth_offset,
                egui::emath::OrderedFloat(image.textured_rect.options.multiplicative_tint.a()),
            )
        });

        Ok(vec![PickableTexturedRect::to_draw_data(
            ctx.viewer_ctx.render_ctx(),
            &self.data.pickable_rects,
        )?])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl EncodedImageVisualizer {
    fn process_encoded_image(
        &mut self,
        ctx: &QueryContext<'_>,
        results: &HybridResults<'_>,
        spatial_ctx: &SpatialSceneEntityContext<'_>,
    ) {
        use super::entity_iterator::iter_slices;
        use re_view::RangeResultsExt as _;

        let entity_path = ctx.target_entity_path;

        let Some(all_blob_chunks) = results.get_required_chunks(EncodedImage::descriptor_blob())
        else {
            return;
        };

        let timeline = ctx.query.timeline();
        let all_blobs_indexed = iter_slices::<&[u8]>(&all_blob_chunks, timeline);
        let all_media_types = results.iter_as(timeline, EncodedImage::descriptor_media_type());
        let all_opacities = results.iter_as(timeline, EncodedImage::descriptor_opacity());

        for ((_time, tensor_data_row_id), blobs, media_types, opacities) in re_query::range_zip_1x2(
            all_blobs_indexed,
            all_media_types.slice::<String>(),
            all_opacities.slice::<f32>(),
        ) {
            let Some(blob) = blobs.first() else {
                continue;
            };
            let media_type = media_types
                .and_then(|media_types| media_types.first().cloned())
                .map(|media_type| MediaType(media_type.into()));

            let image = ctx
                .viewer_ctx
                .store_context
                .caches
                .entry(|c: &mut ImageDecodeCache| {
                    c.entry(
                        tensor_data_row_id,
                        &EncodedImage::descriptor_blob(),
                        blob,
                        media_type.as_ref(),
                    )
                });

            let image = match image {
                Ok(image) => image,
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to decode EncodedImage at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            let opacity: Option<&Opacity> =
                opacities.and_then(|opacity| opacity.first().map(bytemuck::cast_ref));
            let opacity = opacity.copied().unwrap_or_else(|| self.fallback_for(ctx));
            let multiplicative_tint =
                re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));
            let colormap = None;

            if let Some(textured_rect) = textured_rect_from_image(
                ctx.viewer_ctx,
                entity_path,
                spatial_ctx,
                &image,
                colormap,
                multiplicative_tint,
                "EncodedImage",
                &mut self.data,
            ) {
                self.data.pickable_rects.push(PickableTexturedRect {
                    ent_path: entity_path.clone(),
                    textured_rect,
                    source_data: PickableRectSourceData::Image {
                        image,
                        depth_meter: None,
                    },
                });
            }
        }
    }
}

impl TypedComponentFallbackProvider<Opacity> for EncodedImageVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Opacity {
        // Color images should be transparent whenever they're on top of other images,
        // But fully opaque if there are no other images in the scene.
        let Some(view_state) = ctx.view_state.as_any().downcast_ref::<SpatialViewState>() else {
            return 1.0.into();
        };

        // Known cosmetic issues with this approach:
        // * The first frame we have more than one image, the image will be opaque.
        //      It's too complex to do a full view query just for this here.
        //      However, we should be able to analyze the `DataQueryResults` instead to check how many entities are fed to the Image/DepthImage visualizers.
        // * In 3D scenes, images that are on a completely different plane will cause this to become transparent.
        view_state
            .fallback_opacity_for_image_kind(ImageKind::Color)
            .into()
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for EncodedImageVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_IMAGE
    }
}

re_viewer_context::impl_component_fallback_provider!(EncodedImageVisualizer => [DrawOrder, Opacity]);
