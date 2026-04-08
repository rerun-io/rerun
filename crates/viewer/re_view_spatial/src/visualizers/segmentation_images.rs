use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::SegmentationImage;
use re_sdk_types::components::{ImageBuffer, ImageFormat, MagnificationFilter, Opacity};
use re_sdk_types::image::ImageKind;
use re_viewer_context::{
    IdentifiedViewSystem, ImageInfo, ViewClass as _, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerReportSeverity, VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use crate::visualizers::textured_rect_from_image;
use crate::{PickableRectSourceData, PickableTexturedRect};

#[derive(Default)]
pub struct SegmentationImageVisualizer;

struct SegmentationImageComponentData {
    image: ImageInfo,
    opacity: Option<Opacity>,
}

impl IdentifiedViewSystem for SegmentationImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SegmentationImage".into()
    }
}

impl VisualizerSystem for SegmentationImageVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::buffer_and_format::<ImageBuffer, ImageFormat>(
            &SegmentationImage::descriptor_buffer(),
            &SegmentationImage::descriptor_format(),
            &SegmentationImage::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView2D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();

        use super::entity_iterator::process_archetype;
        process_archetype::<SegmentationImage, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let entity_path = ctx.target_entity_path;

                let all_buffers =
                    results.iter_required(SegmentationImage::descriptor_buffer().component);
                if all_buffers.is_empty() {
                    return Ok(());
                }
                let all_formats =
                    results.iter_required(SegmentationImage::descriptor_format().component);
                if all_formats.is_empty() {
                    return Ok(());
                }
                let all_opacities =
                    results.iter_optional(SegmentationImage::descriptor_opacity().component);

                let image_data = re_query::range_zip_1x2(
                    all_buffers.slice::<&[u8]>(),
                    all_formats.component_slow::<ImageFormat>(),
                    all_opacities.slice::<f32>(),
                )
                .filter_map(|((_time, row_id), buffers, formats, opacity)| {
                    let buffer = buffers.first()?;
                    Some(SegmentationImageComponentData {
                        image: ImageInfo::from_stored_blob(
                            row_id,
                            SegmentationImage::descriptor_buffer().component,
                            buffer.clone().into(),
                            first_copied(formats.as_deref())?.0,
                            ImageKind::Segmentation,
                        ),
                        opacity: first_copied(opacity).map(Into::into),
                    })
                });

                for image_data in image_data {
                    let SegmentationImageComponentData { image, opacity } = image_data;

                    let opacity = opacity.unwrap_or_else(|| {
                        typed_fallback_for(ctx, SegmentationImage::descriptor_opacity().component)
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
                        MagnificationFilter::default(),
                        SegmentationImage::name(),
                    ) {
                        Ok(textured_rect) => {
                            data.add_pickable_rect(
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
                            results.report_for_component(
                                SegmentationImage::descriptor_buffer().component,
                                VisualizerReportSeverity::Error,
                                re_error::format(err),
                            );
                        }
                    }
                }
                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([PickableTexturedRect::to_draw_data(
                ctx.viewer_ctx.render_ctx(),
                &data.pickable_rects,
            )?])
            .with_visualizer_data(data))
    }
}

fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
