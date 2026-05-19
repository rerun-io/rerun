use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::EncodedImage;
use re_sdk_types::components::Blob;
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewClass as _, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use crate::visualizers::video::execute_video_stream_like;

#[derive(Default)]
pub struct EncodedImageVisualizer;

impl IdentifiedViewSystem for EncodedImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "EncodedImage".into()
    }
}

impl VisualizerSystem for EncodedImageVisualizer {
    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView2D::identifier())
    }

    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Blob>(
            &EncodedImage::descriptor_blob(),
            &EncodedImage::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let mut data = SpatialViewVisualizerData::default();

        let arch_name = EncodedImage::name();
        let sample_component = EncodedImage::descriptor_blob().component;
        let opacity_component = EncodedImage::descriptor_opacity().component;

        let get_codec: &crate::visualizers::video::GetCodecFn =
            &|ctx, latest_at, data_result, instruction, output| {
                let codec_component = EncodedImage::descriptor_media_type().component;
                let codec_result_wrapped = re_view::BlueprintResolvedResults::LatestAt(
                    latest_at.clone(),
                    data_result.latest_at_with_blueprint_resolved_data_for_component(
                        ctx,
                        latest_at,
                        codec_component,
                        Some(instruction),
                    ),
                );
                let codec_result = re_view::VisualizerInstructionQueryResults::new(
                    instruction,
                    &codec_result_wrapped,
                    output,
                );

                let all_codecs = codec_result.iter_optional(codec_component);
                let codec = all_codecs
                    .slice::<String>()
                    .next()
                    .and_then(|((_time, _row_id), codec)| Some(codec.first()?.to_string()));

                Ok(re_video::VideoCodec::ImageSequence(codec))
            };

        execute_video_stream_like(
            ctx,
            view_query,
            context_systems,
            &mut data,
            Self::identifier(),
            arch_name,
            sample_component,
            opacity_component,
            &get_codec,
        )
    }
}
