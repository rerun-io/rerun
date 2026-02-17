use std::sync::Arc;

use re_log_types::EntityPath;
use re_renderer::external::re_video::VideoLoadError;
use re_renderer::video::Video;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::{AssetVideo, VideoFrameReference};
use re_sdk_types::components::{Blob, MediaType, Opacity, VideoTimestamp};
use re_viewer_context::{
    IdentifiedViewSystem, VideoAssetCache, ViewContext, ViewContextCollection, ViewId, ViewQuery,
    ViewSystemExecutionError, ViewerContext, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use crate::PickableTexturedRect;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::SpatialViewVisualizerData;
use crate::visualizers::entity_iterator::process_archetype;
use crate::visualizers::video::{
    VideoPlaybackIssueSeverity, show_video_playback_issue, video_stream_id,
    visualize_video_frame_texture,
};

pub struct VideoFrameReferenceVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for VideoFrameReferenceVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

impl IdentifiedViewSystem for VideoFrameReferenceVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "VideoFrameReference".into()
    }
}

impl VisualizerSystem for VideoFrameReferenceVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<VideoFrameReference>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        process_archetype::<Self, VideoFrameReference, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                // TODO(andreas): Should ignore range queries here and only do latest-at.
                // Not only would this simplify the code here quite a bit, it would also avoid lots of overhead.
                // Same is true for the image visualizers in general - there seems to be no practical reason to do range queries
                // for visualization here.

                let entity_path = ctx.target_entity_path;

                let all_video_timestamps =
                    results.iter_required(VideoFrameReference::descriptor_timestamp().component);
                if all_video_timestamps.is_empty() {
                    return Ok(());
                }
                let all_video_references = results
                    .iter_optional(VideoFrameReference::descriptor_video_reference().component);
                let all_opacities =
                    results.iter_optional(VideoFrameReference::descriptor_opacity().component);

                for (_index, video_timestamps, video_references, opacity) in re_query::range_zip_1x2(
                    all_video_timestamps.component_slow(),
                    all_video_references.slice::<String>(),
                    all_opacities.slice::<f32>(),
                ) {
                    let Some(video_timestamp): Option<&VideoTimestamp> = video_timestamps.first()
                    else {
                        continue;
                    };

                    self.process_video_frame(
                        ctx,
                        spatial_ctx,
                        video_timestamp,
                        video_references,
                        opacity
                            .and_then(|slice| slice.first())
                            .copied()
                            .map(Opacity::from)
                            .unwrap_or_else(|| {
                                typed_fallback_for(
                                    ctx,
                                    VideoFrameReference::descriptor_opacity().component,
                                )
                            }),
                        entity_path,
                        view_query.view_id,
                    );
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

impl VideoFrameReferenceVisualizer {
    #[expect(clippy::too_many_arguments)]
    fn process_video_frame(
        &mut self,
        ctx: &re_viewer_context::QueryContext<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
        video_timestamp: &VideoTimestamp,
        video_references: Option<Vec<re_sdk_types::ArrowString>>,
        opacity: Opacity,
        entity_path: &EntityPath,
        view_id: ViewId,
    ) {
        re_tracing::profile_function!();

        let player_stream_id = video_stream_id(entity_path, view_id, Self::identifier());

        // Follow the reference to the video asset.
        let video_reference: EntityPath = video_references
            .and_then(|v| v.first().map(|e| e.as_str().into()))
            .unwrap_or_else(|| ctx.target_entity_path.clone());
        let query_result = latest_at_query_video_from_datastore(ctx.viewer_ctx(), &video_reference);

        let world_from_entity = spatial_ctx
            .transform_info
            .single_transform_required_for_entity(
                ctx.target_entity_path,
                VideoFrameReference::name(),
            )
            .as_affine3a();

        // Note that we may or may not know the video size independently of error occurrence.
        // (if it's just a decoding error we may still know the size from the container!)
        // In case we haven error we want to center the message in the middle, so we need some area.
        // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
        let mut video_resolution = glam::vec2(1280.0, 720.0);

        match query_result {
            None => {
                show_video_playback_issue(
                    ctx.view_ctx,
                    &mut self.data,
                    spatial_ctx.highlight,
                    world_from_entity,
                    format!("No video asset at {video_reference:?}"),
                    VideoPlaybackIssueSeverity::Informational,
                    video_resolution,
                    entity_path,
                    spatial_ctx.visualizer_instruction,
                );
            }

            Some((video, video_buffer)) => match video.as_ref() {
                Ok(video) => {
                    if let Some([w, h]) = video.dimensions() {
                        video_resolution = glam::vec2(w as _, h as _);
                    }

                    let video_time = re_viewer_context::video_timestamp_component_to_video_time(
                        ctx.viewer_ctx(),
                        *video_timestamp,
                        video.data_descr().timescale,
                    );

                    match video.frame_at(ctx.render_ctx(), player_stream_id, video_time, &|_| {
                        &video_buffer
                    }) {
                        Ok(video_frame_reference) => {
                            #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
                            let multiplicative_tint =
                                re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));
                            visualize_video_frame_texture(
                                ctx.view_ctx,
                                &mut self.data,
                                video_frame_reference,
                                entity_path,
                                spatial_ctx.depth_offset,
                                world_from_entity,
                                spatial_ctx.highlight,
                                video_resolution,
                                multiplicative_tint,
                            );
                        }

                        Err(err) => {
                            if err.should_request_more_frames() {
                                ctx.view_ctx.egui_ctx().request_repaint();
                            }
                            show_video_playback_issue(
                                ctx.view_ctx,
                                &mut self.data,
                                spatial_ctx.highlight,
                                world_from_entity,
                                err.to_string(),
                                // We don't want to show loading for this since
                                // the data comes from a blob that will always
                                // either be fully loaded or not.
                                err.severity().loading_to_informational(),
                                video_resolution,
                                entity_path,
                                spatial_ctx.visualizer_instruction,
                            );
                        }
                    }
                }
                Err(err) => {
                    show_video_playback_issue(
                        ctx.view_ctx,
                        &mut self.data,
                        spatial_ctx.highlight,
                        world_from_entity,
                        err.to_string(),
                        VideoPlaybackIssueSeverity::Error,
                        video_resolution,
                        entity_path,
                        spatial_ctx.visualizer_instruction,
                    );
                }
            },
        }
    }
}

/// Queries a video from the datstore and caches it in the video cache.
///
/// Note that this does *NOT* check the blueprint store at all.
/// For this, we'd need a [`re_viewer_context::DataResult`] instead of merely a [`EntityPath`].
///
/// Returns `None` if there was no blob at the referenced path.
/// Returns `Some(Err(_))` if there was a blob but it failed to load for some reason.
/// Errors are cached as well so loading a failed video won't occur a high cost repeatedly.
fn latest_at_query_video_from_datastore(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
) -> Option<(Arc<Result<Video, VideoLoadError>>, Blob)> {
    let query = ctx.current_query();

    let results = ctx.recording_engine().cache().latest_at(
        &query,
        entity_path,
        AssetVideo::all_component_identifiers(),
    );

    let blob_row_id = results.component_row_id(AssetVideo::descriptor_blob().component)?;
    let blob = results.component_instance::<Blob>(0, AssetVideo::descriptor_blob().component)?;
    let media_type =
        results.component_instance::<MediaType>(0, AssetVideo::descriptor_media_type().component);

    let video = ctx.store_context.caches.entry(|c: &mut VideoAssetCache| {
        let debug_name = entity_path.to_string();
        c.entry(
            debug_name,
            blob_row_id,
            AssetVideo::descriptor_blob().component,
            &blob,
            media_type.as_ref(),
            ctx.app_options().video_decoder_settings(),
        )
    });
    Some((video, blob))
}
