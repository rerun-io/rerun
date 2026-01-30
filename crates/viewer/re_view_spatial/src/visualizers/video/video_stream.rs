use re_renderer::video::{InsufficientSampleDataError, VideoPlayerError};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::Opacity;
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, VideoStreamCache, VideoStreamProcessingError, ViewClass as _,
    ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
    video_stream_time_from_query,
};

use crate::contexts::{EntityDepthOffsets, TransformTreeContext};
use crate::view_kind::SpatialViewKind;
use crate::visualizers::SpatialViewVisualizerData;
use crate::visualizers::utilities::{
    spatial_view_kind_from_view_class, transform_info_for_archetype_or_report_error,
};
use crate::visualizers::video::{
    VideoPlaybackIssueSeverity, show_video_playback_issue, video_stream_id,
    visualize_video_frame_texture,
};
use crate::{PickableTexturedRect, SpatialView2D};

pub struct VideoStreamVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for VideoStreamVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
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
        VisualizerQueryInfo::from_archetype::<VideoStream>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let mut output = VisualizerExecutionOutput::default();

        let viewer_ctx = ctx.viewer_ctx;
        let view_kind = spatial_view_kind_from_view_class(ctx.view_class_identifier);
        let transforms = context_systems.get::<TransformTreeContext>()?;
        let depth_offsets = context_systems.get::<EntityDepthOffsets>()?;
        let latest_at = view_query.latest_at_query();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let entity_path = &data_result.entity_path;
            re_tracing::profile_scope!("Entity", entity_path.to_string().as_str());

            let Some(transform_info) = transform_info_for_archetype_or_report_error(
                entity_path,
                transforms,
                self.data.preferred_view_kind,
                view_kind,
                &instruction.id,
                &mut output,
            ) else {
                continue;
            };

            let world_from_entity = transform_info
                .single_transform_required_for_entity(entity_path, VideoStream::name())
                .as_affine3a();
            let query_context = ctx.query_context(data_result, &latest_at);
            let highlight = view_query
                .highlights
                .entity_outline_mask(entity_path.hash());

            // Note that we may or may not know the video size independently of error occurrence.
            // (if it's just a decoding error we may still know the size from the container!)
            // In case we haven error we want to center the message in the middle, so we need some area.
            // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
            let mut video_resolution = glam::vec2(1280.0, 720.0);

            let opacity_result = data_result.latest_at_with_blueprint_resolved_data_for_component(
                ctx,
                &latest_at,
                VideoStream::descriptor_opacity().component,
                Some(instruction),
            );
            let all_opacities = opacity_result.iter_as(
                |error| output.report_warning_for(instruction.id, error),
                view_query.timeline,
                VideoStream::descriptor_opacity().component,
            );
            let opacity = all_opacities
                .slice::<f32>()
                .next()
                .and_then(|((_time, _row_id), opacity)| opacity.first())
                .copied()
                .map(Opacity::from);

            let video = match viewer_ctx
                .store_context
                .caches
                .entry(|c: &mut VideoStreamCache| {
                    c.entry(
                        viewer_ctx.recording(),
                        entity_path,
                        view_query.timeline,
                        viewer_ctx.app_options().video_decoder_settings(),
                    )
                }) {
                Ok(video) => video,

                Err(err) => {
                    let (description, severity) = match err {
                        VideoStreamProcessingError::NoVideoSamplesFound => (
                            format!("No video samples available for {entity_path:?}"),
                            VideoPlaybackIssueSeverity::Informational,
                        ),
                        _ => (
                            format!("Failed to play video at {entity_path:?}: {err}"),
                            VideoPlaybackIssueSeverity::Error,
                        ),
                    };

                    show_video_playback_issue(
                        ctx,
                        &mut self.data,
                        highlight,
                        world_from_entity,
                        description,
                        severity,
                        video_resolution,
                        entity_path,
                    );
                    continue;
                }
            };

            let video_time = video_stream_time_from_query(query_context.query);
            if video_time.0 < 0 {
                // The frame is from before the video starts, so nothing to draw here!
                continue;
            }

            let frame_result = {
                let video = video.read();

                if let Some([w, h]) = video.video_renderer.dimensions() {
                    video_resolution = glam::vec2(w as _, h as _);
                }

                let storage_engine = ctx.viewer_ctx.store_context.recording.storage_engine();
                let get_chunk_array = |id| {
                    let chunk = storage_engine
                        .store()
                        .use_physical_chunk_or_report_missing(&id)?;

                    let sample_component = VideoStream::descriptor_sample().component;

                    let (_, buffer) = re_arrow_util::blob_arrays_offsets_and_buffer(
                        chunk.raw_component_array(sample_component)?,
                    )?;

                    Some(buffer)
                };

                video.video_renderer.frame_at(
                    ctx.viewer_ctx.render_ctx(),
                    video_stream_id(entity_path, ctx.view_id, Self::identifier()),
                    video_stream_time_from_query(query_context.query),
                    &|id| {
                        let buffer = get_chunk_array(re_sdk_types::ChunkId::from_tuid(id));

                        buffer.map(|b| b.as_slice()).unwrap_or(&[])
                    },
                )
            };

            match frame_result {
                Ok(frame_texture) => {
                    let depth_offset = depth_offsets
                        .per_entity_and_visualizer
                        .get(&(Self::identifier(), entity_path.hash()))
                        .copied()
                        .unwrap_or_default();
                    let opacity = opacity.unwrap_or_else(|| {
                        typed_fallback_for(
                            &re_viewer_context::QueryContext {
                                view_ctx: ctx,
                                target_entity_path: entity_path,
                                archetype_name: Some(VideoStream::name()),
                                query: &latest_at,
                            },
                            VideoStream::descriptor_opacity().component,
                        )
                    });
                    #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
                    let multiplicative_tint =
                        egui::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));
                    visualize_video_frame_texture(
                        ctx,
                        &mut self.data,
                        frame_texture,
                        entity_path,
                        depth_offset,
                        world_from_entity,
                        highlight,
                        video_resolution,
                        multiplicative_tint,
                    );
                }

                Err(err) => {
                    let severity = match err {
                        VideoPlayerError::InsufficientSampleData(
                            InsufficientSampleDataError::NoKeyFrames
                            | InsufficientSampleDataError::NoSamples
                            | InsufficientSampleDataError::NoLoadedSamples
                            | InsufficientSampleDataError::ExpectedSampleNotLoaded
                            | InsufficientSampleDataError::NoSamplesPriorToRequestedTimestamp,
                        ) => VideoPlaybackIssueSeverity::Informational,
                        _ => VideoPlaybackIssueSeverity::Error,
                    };

                    show_video_playback_issue(
                        ctx,
                        &mut self.data,
                        highlight,
                        world_from_entity,
                        err.to_string(),
                        severity,
                        video_resolution,
                        entity_path,
                    );
                }
            }

            if context_systems.view_class_identifier == SpatialView2D::identifier() {
                let bounding_box = macaw::BoundingBox::from_min_size(
                    world_from_entity.transform_point3(glam::Vec3::ZERO),
                    video_resolution.extend(0.0),
                );
                self.data
                    .add_bounding_box(entity_path.hash(), bounding_box, world_from_entity);
            }
        }

        Ok(output.with_draw_data([PickableTexturedRect::to_draw_data(
            viewer_ctx.render_ctx(),
            &self.data.pickable_rects,
        )?]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
