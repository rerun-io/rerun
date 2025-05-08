use re_log_types::hash::Hash64;
use re_types::{
    archetypes::SegmentationImage,
    components::{DrawOrder, ImageFormat, Opacity},
    image::ImageKind,
    Component as _,
};
use re_viewer_context::{
    IdentifiedViewSystem, ImageInfo, MaybeVisualizableEntities, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::{filter_visualizable_2d_entities, textured_rect_from_image},
    PickableRectSourceData, PickableTexturedRect,
};

use super::SpatialViewVisualizerData;

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
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<SegmentationImage>()
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
        use super::entity_iterator::{iter_component, iter_slices, process_archetype};
        process_archetype::<Self, SegmentationImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let entity_path = ctx.target_entity_path;

                let Some(all_buffer_chunks) =
                    results.get_required_chunks(SegmentationImage::descriptor_buffer())
                else {
                    return Ok(());
                };
                let Some(all_formats_chunks) =
                    results.get_required_chunks(SegmentationImage::descriptor_format())
                else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_buffers_indexed = iter_slices::<&[u8]>(&all_buffer_chunks, timeline);
                let all_formats_indexed =
                    iter_component::<ImageFormat>(&all_formats_chunks, timeline);
                let all_opacities =
                    results.iter_as(timeline, SegmentationImage::descriptor_opacity());

                let data = re_query::range_zip_1x2(
                    all_buffers_indexed,
                    all_formats_indexed,
                    all_opacities.slice::<f32>(),
                )
                .filter_map(|(index, buffers, formats, opacity)| {
                    let buffer = buffers.first()?;
                    Some(SegmentationImageComponentData {
                        image: ImageInfo {
                            buffer_cache_key: Hash64::hash(index.1),
                            buffer: buffer.clone().into(),
                            format: first_copied(formats.as_deref())?.0,
                            kind: ImageKind::Segmentation,
                        },
                        opacity: first_copied(opacity).map(Into::into),
                    })
                });

                for data in data {
                    let SegmentationImageComponentData { image, opacity } = data;

                    let opacity = opacity.unwrap_or_else(|| self.fallback_for(ctx));
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
                        "SegmentationImage",
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

impl TypedComponentFallbackProvider<Opacity> for SegmentationImageVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Opacity {
        // Segmentation images should be transparent whenever they're on top of other images,
        // But fully opaque if there are no other images in the scene.
        let Some(view_state) = ctx.view_state.as_any().downcast_ref::<SpatialViewState>() else {
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

fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
