use itertools::Itertools as _;

use re_space_view::HybridResults;
use re_types::{
    archetypes::Image,
    components::{
        Blob, ChannelDatatype, ColorModel, DrawOrder, Opacity, PixelFormat, Resolution2D,
    },
    image::ImageKind,
    Loggable as _,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ImageFormat, ImageInfo, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_2d_entities, textured_rect_from_image},
    PickableImageRect,
};

use super::{entity_iterator::process_archetype, SpatialViewVisualizerData};

pub struct ImageVisualizer {
    pub data: SpatialViewVisualizerData,
    pub images: Vec<PickableImageRect>,
}

impl Default for ImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
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
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Image>()
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

        process_archetype::<Self, Image, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                self.process_image(ctx, results, spatial_ctx);
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

impl ImageVisualizer {
    fn process_image(
        &mut self,
        ctx: &QueryContext<'_>,
        results: &HybridResults<'_>,
        spatial_ctx: &SpatialSceneEntityContext<'_>,
    ) {
        use super::entity_iterator::{iter_buffer, iter_primitive_array};
        use re_space_view::RangeResultsExt as _;

        let entity_path = ctx.target_entity_path;

        let Some(all_blob_chunks) = results.get_required_chunks(&Blob::name()) else {
            return;
        };
        let Some(all_resolution_chunks) = results.get_required_chunks(&Resolution2D::name()) else {
            return;
        };

        let timeline = ctx.query.timeline();
        let all_blobs_indexed = iter_buffer::<u8>(&all_blob_chunks, timeline, Blob::name());
        let all_resolutions_indexed =
            iter_primitive_array(&all_resolution_chunks, timeline, Resolution2D::name());
        let all_pixel_formats = results.iter_as(timeline, PixelFormat::name());
        let all_color_models = results.iter_as(timeline, ColorModel::name());
        let all_channel_datatypes = results.iter_as(timeline, ChannelDatatype::name());
        let all_opacities = results.iter_as(timeline, Opacity::name());

        let data = re_query::range_zip_1x5(
            all_blobs_indexed,
            all_resolutions_indexed,
            all_pixel_formats.component::<PixelFormat>(),
            all_color_models.component::<ColorModel>(),
            all_channel_datatypes.component::<ChannelDatatype>(),
            all_opacities.primitive::<f32>(),
        )
        .filter_map(
            |(index, blobs, resolutions, pixel_formats, color_models, datatypes, opacities)| {
                let blob = blobs.first()?.0.clone();

                let format = if let Some(pixel_format) = first_copied(pixel_formats.as_deref()) {
                    ImageFormat::PixelFormat(pixel_format)
                } else {
                    let color_model = first_copied(color_models.as_deref())?;
                    let datatype = first_copied(datatypes.as_deref())?;
                    ImageFormat::ColorModel {
                        color_model,
                        datatype,
                    }
                };

                Some(ImageComponentData {
                    image: ImageInfo {
                        blob_row_id: index.1,
                        blob: re_types::datatypes::Blob(blob.into()),
                        resolution: first_copied(resolutions)?,
                        format,
                        kind: ImageKind::Color,
                        colormap: None,
                    },
                    opacity: first_copied(opacities).map(Into::into),
                })
            },
        );

        for ImageComponentData { image, opacity } in data {
            let opacity = opacity.unwrap_or_else(|| self.fallback_for(ctx));
            let multiplicative_tint =
                re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));

            if let Some(textured_rect) = textured_rect_from_image(
                ctx.viewer_ctx,
                entity_path,
                spatial_ctx,
                &image,
                multiplicative_tint,
                "Image",
                &mut self.data,
            ) {
                self.images.push(PickableImageRect {
                    ent_path: entity_path.clone(),
                    image,
                    textured_rect,
                    depth_meter: None,
                });
            }
        }
    }
}

impl TypedComponentFallbackProvider<Opacity> for ImageVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Opacity {
        1.0.into()
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for ImageVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_IMAGE
    }
}

re_viewer_context::impl_component_fallback_provider!(ImageVisualizer => [DrawOrder, Opacity]);

fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
