use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::SegmentationImage;
use re_sdk_types::components::{ImageFormat, Opacity};
use re_sdk_types::image::ImageKind;
use re_viewer_context::{
    IdentifiedViewSystem, ImageInfo, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::textured_rect_from_image;
use crate::{PickableRectSourceData, PickableTexturedRect};

pub struct SegmentationImageVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for SegmentationImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

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
        VisualizerQueryInfo::from_archetype::<SegmentationImage>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        use super::entity_iterator::{iter_component, iter_slices, process_archetype};
        process_archetype::<Self, SegmentationImage, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let entity_path = ctx.target_entity_path;

                let all_buffer_chunks =
                    results.get_required_chunk(SegmentationImage::descriptor_buffer().component);
                if all_buffer_chunks.is_empty() {
                    return Ok(());
                }
                let all_formats_chunks =
                    results.get_required_chunk(SegmentationImage::descriptor_format().component);
                if all_formats_chunks.is_empty() {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_buffers_indexed = iter_slices::<&[u8]>(&all_buffer_chunks, timeline);
                let all_formats_indexed =
                    iter_component::<ImageFormat>(&all_formats_chunks, timeline);
                let all_opacities =
                    results.iter_as(timeline, SegmentationImage::descriptor_opacity().component);

                let data = re_query::range_zip_1x2(
                    all_buffers_indexed,
                    all_formats_indexed,
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

                for data in data {
                    let SegmentationImageComponentData { image, opacity } = data;

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
                        SegmentationImage::name(),
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
                            spatial_ctx
                                .output
                                .report_error_for(entity_path.clone(), re_error::format(err));
                        }
                    }
                }
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

fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
