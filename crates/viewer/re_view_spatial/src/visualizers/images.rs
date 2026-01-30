use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Image;
use re_sdk_types::components::{ImageFormat, Opacity};
use re_sdk_types::image::ImageKind;
use re_view::HybridResults;
use re_viewer_context::{
    IdentifiedViewSystem, ImageInfo, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{first_copied, textured_rect_from_image};
use crate::{PickableRectSourceData, PickableTexturedRect};

pub struct ImageVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for ImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

struct ImageComponentData {
    image: ImageInfo,
    opacity: Option<Opacity>,
}

impl IdentifiedViewSystem for ImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Image".into()
    }
}

impl VisualizerSystem for ImageVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Image>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let mut output = VisualizerExecutionOutput::default();

        process_archetype::<Self, Image, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                self.process_image(ctx, results, spatial_ctx);
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

impl ImageVisualizer {
    fn process_image(
        &mut self,
        ctx: &QueryContext<'_>,
        results: &HybridResults<'_>,
        spatial_ctx: &mut SpatialSceneVisualizerInstructionContext<'_>,
    ) {
        re_tracing::profile_function!();

        use re_view::RangeResultsExt as _;

        use super::entity_iterator::{iter_component, iter_slices};

        let entity_path = ctx.target_entity_path;

        let all_buffer_chunks = results
            .get_required_chunk(Image::descriptor_buffer().component)
            .ensure_required(|err| spatial_ctx.report_error(err));
        if all_buffer_chunks.is_empty() {
            return;
        }
        let all_formats_chunks = results
            .get_required_chunk(Image::descriptor_format().component)
            .ensure_required(|err| spatial_ctx.report_error(err));
        if all_formats_chunks.is_empty() {
            return;
        }

        let timeline = ctx.query.timeline();
        let all_buffers_indexed = iter_slices::<&[u8]>(&all_buffer_chunks, timeline);
        let all_formats_indexed = iter_component::<ImageFormat>(&all_formats_chunks, timeline);
        let all_opacities = results.iter_as(
            |err| spatial_ctx.report_warning(err),
            timeline,
            Image::descriptor_opacity().component,
        );

        let data = re_query::range_zip_1x2(
            all_buffers_indexed,
            all_formats_indexed,
            all_opacities.slice::<f32>(),
        )
        .filter_map(|((_time, row_id), buffers, formats, opacities)| {
            let buffer = buffers.first()?;

            Some(ImageComponentData {
                image: ImageInfo::from_stored_blob(
                    row_id,
                    Image::descriptor_buffer().component,
                    buffer.clone().into(),
                    first_copied(formats.as_deref())?.0,
                    ImageKind::Color,
                ),
                opacity: first_copied(opacities).map(Into::into),
            })
        });

        for ImageComponentData { image, opacity } in data {
            let opacity = opacity
                .unwrap_or_else(|| typed_fallback_for(ctx, Image::descriptor_opacity().component));
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
                Image::name(),
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
                    spatial_ctx.report_error(re_error::format(err));
                }
            }
        }
    }
}
