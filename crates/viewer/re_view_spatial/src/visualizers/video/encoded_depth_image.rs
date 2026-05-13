use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::EncodedDepthImage;
use re_sdk_types::components::{Blob, Colormap, DepthMeter, FillRatio, MediaType, ValueRange};
use re_view::{DataResultQuery as _, latest_at_with_blueprint_resolved_data};
use re_viewer_context::ViewClass as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    gpu_bridge::colormap_to_re_renderer, typed_fallback_for,
};

use super::{DepthTextureConfig, SpatialViewVisualizerData, execute_video_stream_like};
use crate::visualizers::depth_images::DepthImageProcessResult;
use crate::visualizers::video::VideoStreamCtx;

pub struct EncodedDepthImageVisualizerOutput {
    /// Depth cloud entities, keyed by entity path, for picking.
    ///
    /// Currently always empty; will be populated once depth cloud rendering is wired
    /// up through the video path.
    pub depth_cloud_entities: IntMap<EntityPathHash, DepthImageProcessResult>,
}

#[derive(Default)]
pub struct EncodedDepthImageVisualizer;

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
        VisualizerQueryInfo::single_required_component::<Blob>(
            &EncodedDepthImage::descriptor_blob(),
            &EncodedDepthImage::all_components(),
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
        re_tracing::profile_function!();

        let mut data = SpatialViewVisualizerData::default();
        let mut depth_cloud_entities: IntMap<EntityPathHash, DepthImageProcessResult> =
            IntMap::default();

        let arch_name = EncodedDepthImage::name();
        let sample_component = EncodedDepthImage::descriptor_blob().component;

        let get_codec: &crate::visualizers::video::GetCodecFn =
            &|ctx, latest_at, data_result, instruction, output| {
                let codec_component = EncodedDepthImage::descriptor_media_type().component;
                let results = data_result.latest_at_with_blueprint_resolved_data_for_component(
                    ctx,
                    latest_at,
                    codec_component,
                    Some(instruction),
                );
                if results.any_missing_chunks() {
                    output.set_missing_chunks();
                }

                let codec = results
                    .get_mono::<MediaType>(codec_component)
                    .map(|m| m.to_string());
                Ok(re_video::VideoCodec::ImageSequence(codec))
            };

        let get_depth_config: &crate::visualizers::video::GetDepthConfigFn =
            &|ctx, latest_at, data_result, instruction, output| {
                let colormap_component = EncodedDepthImage::descriptor_colormap().component;
                let value_range_component = EncodedDepthImage::descriptor_depth_range().component;
                let depth_meter_component = EncodedDepthImage::descriptor_meter().component;
                let fill_ratio_component =
                    EncodedDepthImage::descriptor_point_fill_ratio().component;

                let query_ctx = re_viewer_context::QueryContext {
                    view_ctx: ctx,
                    target_entity_path: &data_result.entity_path,
                    instruction_id: Some(instruction.id),
                    archetype_name: Some(EncodedDepthImage::name()),
                    query: latest_at.clone(),
                };

                let results = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    latest_at,
                    data_result,
                    [
                        colormap_component,
                        value_range_component,
                        depth_meter_component,
                        fill_ratio_component,
                    ],
                    Some(instruction),
                );
                if results.any_missing_chunks() {
                    output.set_missing_chunks();
                }

                let colormap: Colormap = results
                    .get_mono(colormap_component)
                    .unwrap_or_else(|| typed_fallback_for(&query_ctx, colormap_component));

                let value_range: ValueRange = results
                    .get_mono(value_range_component)
                    .unwrap_or_else(|| typed_fallback_for(&query_ctx, value_range_component));
                let value_range = [value_range.0.0[0] as f32, value_range.0.0[1] as f32];

                let depth_meter: DepthMeter = results
                    .get_mono(depth_meter_component)
                    .unwrap_or_else(|| typed_fallback_for(&query_ctx, depth_meter_component));

                let fill_ratio: FillRatio =
                    results.get_mono(fill_ratio_component).unwrap_or_default();

                DepthTextureConfig {
                    colormap: colormap_to_re_renderer(colormap),
                    range: value_range,
                    depth_meter,
                    fill_ratio,
                }
            };

        let ctx = VideoStreamCtx::new(
            ctx,
            view_query,
            context_systems,
            &mut data,
            Self::identifier(),
            arch_name,
            sample_component,
            &get_codec,
        )
        .with_depth_handler(get_depth_config, &mut depth_cloud_entities);

        let output = execute_video_stream_like(ctx)?;

        Ok(
            output.with_visualizer_data(EncodedDepthImageVisualizerOutput {
                depth_cloud_entities,
            }),
        )
    }
}
