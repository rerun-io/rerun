use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::EncodedImage;
use re_sdk_types::components::{MagnificationFilter, MediaType, Opacity};
use re_view::VisualizerInstructionQueryResults;
use re_viewer_context::{
    IdentifiedViewSystem, ImageDecodeCache, QueryContext, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
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

        let output = VisualizerExecutionOutput::default();

        process_archetype::<Self, EncodedImage, _>(
            ctx,
            view_query,
            context_systems,
            &output,
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
        results: &VisualizerInstructionQueryResults<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
    ) {
        re_tracing::profile_function!();

        let entity_path = ctx.target_entity_path;

        let all_blobs = results.iter_required(EncodedImage::descriptor_blob().component);
        if all_blobs.is_empty() {
            return;
        }
        let all_media_types =
            results.iter_optional(EncodedImage::descriptor_media_type().component);
        let all_opacities = results.iter_optional(EncodedImage::descriptor_opacity().component);
        let all_magnification_filters =
            results.iter_optional(EncodedImage::descriptor_magnification_filter().component);

        for ((_time, tensor_data_row_id), blobs, media_types, opacities, magnification_filters) in
            re_query::range_zip_1x3(
                all_blobs.slice::<&[u8]>(),
                all_media_types.slice::<String>(),
                all_opacities.slice::<f32>(),
                all_magnification_filters.slice::<u8>(),
            )
        {
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

            let magnification_filter = magnification_filters
                .and_then(|f| f.first().copied())
                .and_then(MagnificationFilter::from_u8)
                .unwrap_or_default();

            match textured_rect_from_image(
                ctx.viewer_ctx(),
                entity_path,
                spatial_ctx,
                &image,
                colormap,
                multiplicative_tint,
                magnification_filter,
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
                Err(err) => {
                    results.report_error(re_error::format(err));
                }
            }
        }
    }
}
