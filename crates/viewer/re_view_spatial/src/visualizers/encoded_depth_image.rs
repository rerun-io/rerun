use re_chunk_store::ChunkStoreEvent;
use re_renderer::renderer::{DepthCloud, DepthClouds};
use re_types::{
    Archetype as _,
    archetypes::EncodedDepthImage,
    components::{Colormap, ImageFormat, MediaType},
};
use re_viewer_context::{
    DataBasedVisualizabilityFilter, IdentifiedViewSystem, ImageDecodeCache,
    MaybeVisualizableEntities, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    PickableTexturedRect, contexts::TransformTreeContext, view_kind::SpatialViewKind,
    visualizers::filter_visualizable_2d_entities,
};

use super::{
    SpatialViewVisualizerData,
    depth_images::{
        DepthCloudEntities, DepthImageComponentData, first_copied, process_depth_image_data,
    },
};

pub struct EncodedDepthImageVisualizer {
    pub data: SpatialViewVisualizerData,
    pub depth_cloud_entities: DepthCloudEntities,
}

impl Default for EncodedDepthImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
            depth_cloud_entities: Default::default(),
        }
    }
}

impl IdentifiedViewSystem for EncodedDepthImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "EncodedDepthImage".into()
    }
}

impl VisualizerSystem for EncodedDepthImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<EncodedDepthImage>()
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
        let mut depth_clouds: Vec<DepthCloud> = Vec::new();

        let transforms = context_systems.get::<TransformTreeContext>()?;

        use super::entity_iterator::{iter_component, iter_slices, process_archetype};
        process_archetype::<Self, EncodedDepthImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_blob_chunks) =
                    results.get_required_chunks(EncodedDepthImage::descriptor_blob().component)
                else {
                    return Ok(());
                };
                let Some(all_format_chunks) =
                    results.get_required_chunks(EncodedDepthImage::descriptor_format().component)
                else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_blobs_indexed = iter_slices::<&[u8]>(&all_blob_chunks, timeline);
                let all_formats_indexed =
                    iter_component::<ImageFormat>(&all_format_chunks, timeline);
                let all_media_types = results.iter_as(
                    timeline,
                    EncodedDepthImage::descriptor_media_type().component,
                );
                let all_colormaps =
                    results.iter_as(timeline, EncodedDepthImage::descriptor_colormap().component);
                let all_value_ranges = results.iter_as(
                    timeline,
                    EncodedDepthImage::descriptor_depth_range().component,
                );
                let all_depth_meters =
                    results.iter_as(timeline, EncodedDepthImage::descriptor_meter().component);
                let all_fill_ratios = results.iter_as(
                    timeline,
                    EncodedDepthImage::descriptor_point_fill_ratio().component,
                );

                let entity_path = ctx.target_entity_path;

                let data = re_query::range_zip_1x6(
                    all_blobs_indexed,
                    all_formats_indexed,
                    all_media_types.slice::<String>(),
                    all_colormaps.slice::<u8>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_depth_meters.slice::<f32>(),
                    all_fill_ratios.slice::<f32>(),
                )
                .filter_map(
                    |(
                        (_time, row_id),
                        blobs,
                        format,
                        media_type,
                        colormap,
                        value_range,
                        depth_meter,
                        fill_ratio,
                    )| {
                        let blob = blobs.first()?;
                        let format = first_copied(format.as_deref())?;
                        let media_type = media_type
                            .and_then(|types| types.first().cloned())
                            .map(|mt| MediaType(mt.into()));

                        let image = match ctx.store_ctx().caches.entry(
                            |c: &mut ImageDecodeCache| {
                                c.entry_encoded_depth(
                                    row_id,
                                    EncodedDepthImage::descriptor_blob().component,
                                    blob,
                                    media_type.as_ref(),
                                    &format,
                                )
                            },
                        ) {
                            Ok(image) => image,
                            Err(err) => {
                                re_log::warn_once!(
                                    "Failed to decode EncodedDepthImage at path {entity_path}: {err}"
                                );
                                return None;
                            }
                        };

                        Some(DepthImageComponentData {
                            image,
                            depth_meter: first_copied(depth_meter).map(Into::into),
                            fill_ratio: first_copied(fill_ratio).map(Into::into),
                            colormap: first_copied(colormap).and_then(Colormap::from_u8),
                            value_range: first_copied(value_range),
                        })
                    },
                );

                process_depth_image_data(
                    &mut self.data,
                    &mut self.depth_cloud_entities,
                    ctx,
                    &mut depth_clouds,
                    spatial_ctx,
                    transforms,
                    data,
                    EncodedDepthImage::name(),
                );

                Ok(())
            },
        )?;

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::DepthCloudDrawData::new(
            ctx.viewer_ctx.render_ctx(),
            &DepthClouds {
                clouds: depth_clouds,
                radius_boost_in_ui_points_for_outlines:
                    re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
            },
        ) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!(
                    "Failed to create depth cloud draw data from encoded depth images: {err}"
                );
            }
        }

        draw_data_list.push(PickableTexturedRect::to_draw_data(
            ctx.viewer_ctx.render_ctx(),
            &self.data.pickable_rects,
        )?);

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn data_based_visualizability_filter(&self) -> Option<Box<dyn DataBasedVisualizabilityFilter>> {
        Some(Box::new(EncodedDepthImageVisualizabilityFilter))
    }
}

#[derive(Default)]
struct EncodedDepthImageVisualizabilityFilter;

impl DataBasedVisualizabilityFilter for EncodedDepthImageVisualizabilityFilter {
    fn update_visualizability(&mut self, event: &ChunkStoreEvent) -> bool {
        event
            .diff
            .chunk
            .components()
            .component_descriptors()
            .any(|component_descr| {
                let component = component_descr.component;
                component == EncodedDepthImage::descriptor_blob().component
                    || component == EncodedDepthImage::descriptor_format().component
            })
    }
}
