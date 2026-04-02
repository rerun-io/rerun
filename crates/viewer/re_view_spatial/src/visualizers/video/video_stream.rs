use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, VideoStreamProcessingError, ViewClass as _, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::visualizers::SpatialViewVisualizerData;
use crate::visualizers::video::execute_video_stream_like;

#[derive(Default)]
pub struct VideoStreamVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl IdentifiedViewSystem for VideoStreamVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "VideoStream".into()
    }
}

impl VisualizerSystem for VideoStreamVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<VideoCodec>(
            &VideoStream::descriptor_codec(),
            &VideoStream::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView2D::identifier())
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        execute_video_stream_like(
            ctx,
            view_query,
            context_systems,
            &mut self.data,
            Self::identifier(),
            VideoStream::name(),
            VideoStream::descriptor_sample().component,
            VideoStream::descriptor_opacity().component,
            &|ctx, latest_at, data_result, instruction, output| {
                let codec_component = VideoStream::descriptor_codec().component;
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
                    .slice::<u32>()
                    .next()
                    .and_then(|((_time, _row_id), codec)| {
                        re_sdk_types::components::VideoCodec::try_from_u32(*codec.first()?)
                    })
                    .ok_or(VideoStreamProcessingError::MissingCodec)?;

                Ok(codec.into())
            },
        )
    }
}
