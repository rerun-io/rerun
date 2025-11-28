use re_types::{
    Archetype as _,
    archetypes::EncodedDepthImage,
    components::{Colormap, ImageFormat, MediaType},
};
use re_viewer_context::{
    IdentifiedViewSystem, ImageDecodeCache, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use crate::view_kind::SpatialViewKind;

use super::{
    SpatialViewVisualizerData,
    depth_images::{
        DepthCloudEntities, DepthImageComponentData, execute_depth_visualizer, first_copied,
        process_depth_image_data,
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

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let preferred_view_kind = self.data.preferred_view_kind;

        execute_depth_visualizer::<Self, EncodedDepthImage, _>(
            &mut self.data,
            &mut self.depth_cloud_entities,
            ctx,
            view_query,
            context_systems,
            preferred_view_kind,
            |data, depth_cloud_entities, ctx, spatial_ctx, transforms, depth_clouds, results| {
                use super::entity_iterator::{iter_component, iter_slices};
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

                let data_iter = re_query::range_zip_1x6(
                    all_blobs_indexed,
                    all_formats_indexed,
                    all_media_types.slice::<String>(),
                    all_colormaps.slice::<u8>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_depth_meters.slice::<f32>(),
                    all_fill_ratios.slice::<f32>(),
                )
                .filter_map(
                    move |(
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
                                re_log::warn_once!("Failed to decode EncodedDepthImage at path {entity_path}: {err}");
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
                    data,
                    depth_cloud_entities,
                    ctx,
                    depth_clouds,
                    spatial_ctx,
                    transforms,
                    data_iter,
                    EncodedDepthImage::name(),
                );

                Ok(())
            },
        )
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
