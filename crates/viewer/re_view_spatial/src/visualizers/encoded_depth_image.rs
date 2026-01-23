use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_sdk_types::{
    Archetype as _,
    archetypes::EncodedDepthImage,
    components::{Colormap, MediaType},
};
use re_viewer_context::{
    IdentifiedViewSystem, ImageDecodeCache, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::entity_iterator::process_archetype;
use super::{
    SpatialViewVisualizerData,
    depth_images::{DepthImageComponentData, process_depth_image_data},
};
use crate::{
    contexts::TransformTreeContext,
    view_kind::SpatialViewKind,
    visualizers::{
        depth_images::{DepthImageProcessResult, populate_depth_visualizer_execution_result},
        first_copied,
    },
};

pub struct EncodedDepthImageVisualizer {
    pub data: SpatialViewVisualizerData,

    /// Expose image infos for depth clouds - we need this for picking interaction.
    pub depth_cloud_entities: IntMap<EntityPathHash, DepthImageProcessResult>,
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
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<EncodedDepthImage>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let preferred_view_kind = self.data.preferred_view_kind;
        let mut output = VisualizerExecutionOutput::default();
        let mut depth_clouds = Vec::new();

        let transforms = context_systems.get::<TransformTreeContext>()?;

        process_archetype::<Self, EncodedDepthImage, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use super::entity_iterator::iter_slices;
                use re_view::RangeResultsExt as _;

                let all_blob_chunks =
                    results.get_chunks(EncodedDepthImage::descriptor_blob().component);
                if all_blob_chunks.is_empty() {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_blobs_indexed = iter_slices::<&[u8]>(&all_blob_chunks, timeline);
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

                for (
                    (_time, row_id),
                    blobs,
                    media_type,
                    colormap,
                    value_range,
                    depth_meter,
                    fill_ratio,
                ) in re_query::range_zip_1x5(
                    all_blobs_indexed,
                    all_media_types.slice::<String>(),
                    all_colormaps.slice::<u8>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_depth_meters.slice::<f32>(),
                    all_fill_ratios.slice::<f32>(),
                ) {
                    let Some(blob) = blobs.first() else {
                        spatial_ctx.output.report_error_for(
                            entity_path.clone(),
                            "EncodedDepthImage blob is empty.".to_owned(),
                        );
                        continue;
                    };

                    let media_type = media_type
                        .and_then(|types| types.first().cloned())
                        .map(|mt| MediaType(mt.into()));

                    let image = match ctx.store_ctx().caches.entry(|c: &mut ImageDecodeCache| {
                        c.entry_encoded_depth(
                            row_id,
                            EncodedDepthImage::descriptor_blob().component,
                            blob,
                            media_type.as_ref(),
                        )
                    }) {
                        Ok(image) => image,
                        Err(err) => {
                            spatial_ctx.output.report_error_for(
                                entity_path.clone(),
                                format!("Failed to decode EncodedDepthImage blob: {err}"),
                            );
                            continue;
                        }
                    };

                    let data = DepthImageComponentData {
                        image,
                        depth_meter: first_copied(depth_meter).map(Into::into),
                        fill_ratio: first_copied(fill_ratio).map(Into::into),
                        colormap: first_copied(colormap).and_then(Colormap::from_u8),
                        value_range: first_copied(value_range),
                    };

                    process_depth_image_data(
                        ctx,
                        spatial_ctx,
                        &mut self.data,
                        &mut self.depth_cloud_entities,
                        &mut depth_clouds,
                        transforms,
                        data,
                        EncodedDepthImage::name(),
                        EncodedDepthImage::descriptor_meter().component,
                        EncodedDepthImage::descriptor_colormap().component,
                    );
                }

                Ok(())
            },
        )?;

        populate_depth_visualizer_execution_result(ctx, &self.data, depth_clouds, output)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
