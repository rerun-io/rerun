use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::EncodedImage;
use re_sdk_types::components::{MediaType, Opacity};
use re_view::HybridResults;
use re_viewer_context::{
    IdentifiedViewSystem, ImageDecodeCache, QueryContext, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneEntityContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::textured_rect_from_image;
use crate::{PickableRectSourceData, PickableTexturedRect};

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
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<EncodedImage>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let mut output = VisualizerExecutionOutput::default();

        process_archetype::<Self, EncodedImage, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                self.process_encoded_image(ctx, results, spatial_ctx);
                Ok(())
            },
        )?;

        Ok(output.with_draw_data([PickableTexturedRect::to_draw_data(
            ctx.viewer_ctx.render_ctx(),
            &self.data.pickable_rects,
        )?]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}

impl EncodedImageVisualizer {
    fn process_encoded_image(
        &mut self,
        ctx: &QueryContext<'_>,
        results: &HybridResults<'_>,
        spatial_ctx: &mut SpatialSceneEntityContext<'_>,
    ) {
        re_tracing::profile_function!();

        use re_view::RangeResultsExt as _;

        use super::entity_iterator::iter_slices;

        let entity_path = ctx.target_entity_path;

        let all_blob_chunks = results.get_required_chunk(EncodedImage::descriptor_blob().component);
        if all_blob_chunks.is_empty() {
            return;
        }

        let timeline = ctx.query.timeline();
        let all_blobs_indexed = iter_slices::<&[u8]>(&all_blob_chunks, timeline);
        let all_media_types =
            results.iter_as(timeline, EncodedImage::descriptor_media_type().component);
        let all_opacities = results.iter_as(timeline, EncodedImage::descriptor_opacity().component);

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

            let image = ctx.store_ctx().caches.entry(|c: &mut ImageDecodeCache| {
                c.entry_encoded_color(
                    tensor_data_row_id,
                    EncodedImage::descriptor_blob().component,
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
            let opacity = opacity.copied().unwrap_or_else(|| {
                typed_fallback_for(ctx, EncodedImage::descriptor_opacity().component)
            });
            #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
            let multiplicative_tint =
                re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));
            let colormap = None;

            match textured_rect_from_image(
                ctx.viewer_ctx(),
                entity_path,
                spatial_ctx,
                &image,
                colormap,
                multiplicative_tint,
                EncodedImage::name(),
            ) {
                Ok(textured_rect) => {
                    self.data.add_pickable_rect(
                        PickableTexturedRect {
                            ent_path: entity_path.clone(),
                            textured_rect,
                            source_data: PickableRectSourceData::Image {
                                image,
                                depth_meter: None,
                            },
                        },
                        spatial_ctx.view_class_identifier,
                    );
                }
                Err(err) => spatial_ctx
                    .output
                    .report_error_for(entity_path.clone(), re_error::format(err)),
            }
        }
    }
}
