use std::sync::Arc;

use re_log_types::EntityPath;
use re_renderer::{external::re_video::VideoLoadError, video::Video};
use re_types::{
    Archetype as _,
    archetypes::{AssetVideo, VideoFrameReference},
    components::{self, Blob, MediaType, VideoTimestamp},
};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, TypedComponentFallbackProvider,
    VideoAssetCache, ViewContext, ViewContextCollection, ViewId, ViewQuery,
    ViewSystemExecutionError, ViewerContext, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    PickableTexturedRect,
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialViewKind,
    visualizers::{
        SpatialViewVisualizerData,
        entity_iterator::{self, process_archetype},
        filter_visualizable_2d_entities,
        video::{show_video_error, video_stream_id, visualize_video_frame_texture},
    },
};

// TODO(#9832): Support opacity for videos
// TODO(jan): Fallback opacity in the same way as color/depth/segmentation images
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
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<VideoFrameReference>()
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
        process_archetype::<Self, VideoFrameReference, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                // TODO(andreas): Should ignore range queries here and only do latest-at.
                // Not only would this simplify the code here quite a bit, it would also avoid lots of overhead.
                // Same is true for the image visualizers in general - there seems to be no practical reason to do range queries
                // for visualization here.
                use re_view::RangeResultsExt as _;

                let timeline = ctx.query.timeline();
                let entity_path = ctx.target_entity_path;

                let Some(all_video_timestamp_chunks) =
                    results.get_required_chunks(VideoFrameReference::descriptor_timestamp())
                else {
                    return Ok(());
                };
                let all_video_references =
                    results.iter_as(timeline, VideoFrameReference::descriptor_video_reference());

                for (_index, video_timestamps, video_references) in re_query::range_zip_1x1(
                    entity_iterator::iter_component(&all_video_timestamp_chunks, timeline),
                    all_video_references.slice::<String>(),
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
                        entity_path,
                        view_query.view_id,
                    );
                }

                Ok(())
            },
        )?;

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

impl VideoFrameReferenceVisualizer {
    fn process_video_frame(
        &mut self,
        ctx: &re_viewer_context::QueryContext<'_>,
        spatial_ctx: &SpatialSceneEntityContext<'_>,
        video_timestamp: &VideoTimestamp,
        video_references: Option<Vec<re_types::ArrowString>>,
        entity_path: &EntityPath,
        view_id: ViewId,
    ) {
        re_tracing::profile_function!();

        let player_stream_id = video_stream_id(entity_path, view_id, Self::identifier());

        // Follow the reference to the video asset.
        let video_reference: EntityPath = video_references
            .and_then(|v| v.first().map(|e| e.as_str().into()))
            .unwrap_or_else(|| {
                TypedComponentFallbackProvider::<components::EntityPath>::fallback_for(self, ctx)
                    .as_str()
                    .into()
            });
        let query_result = latest_at_query_video_from_datastore(ctx.viewer_ctx(), &video_reference);

        let world_from_entity = spatial_ctx
            .transform_info
            .single_entity_transform_required(ctx.target_entity_path, VideoFrameReference::name());

        // Note that we may or may not know the video size independently of error occurrence.
        // (if it's just a decoding error we may still know the size from the container!)
        // In case we haven error we want to center the message in the middle, so we need some area.
        // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
        let mut video_resolution = glam::vec2(1280.0, 720.0);

        match query_result {
            None => {
                show_video_error(
                    ctx.view_ctx,
                    &mut self.data,
                    spatial_ctx.highlight,
                    world_from_entity,
                    format!("No video asset at {video_reference:?}"),
                    video_resolution,
                    entity_path,
                );
            }

            Some((video, video_buffer)) => match video.as_ref() {
                Ok(video) => {
                    match video.frame_at(
                        ctx.render_ctx(),
                        player_stream_id,
                        video_timestamp.as_secs(),
                        &std::iter::once(video_buffer.as_ref()).collect(),
                    ) {
                        Ok(video_frame_reference) => {
                            visualize_video_frame_texture(
                                ctx.view_ctx,
                                &mut self.data,
                                video_frame_reference,
                                entity_path,
                                spatial_ctx.depth_offset,
                                world_from_entity,
                                spatial_ctx.highlight,
                                video_resolution,
                            );
                        }

                        Err(err) => {
                            if let Some([w, h]) = video.dimensions() {
                                video_resolution = glam::vec2(w as _, h as _);
                            }
                            show_video_error(
                                ctx.view_ctx,
                                &mut self.data,
                                spatial_ctx.highlight,
                                world_from_entity,
                                err.to_string(),
                                video_resolution,
                                entity_path,
                            );
                        }
                    }
                }
                Err(err) => {
                    show_video_error(
                        ctx.view_ctx,
                        &mut self.data,
                        spatial_ctx.highlight,
                        world_from_entity,
                        err.to_string(),
                        video_resolution,
                        entity_path,
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
        AssetVideo::all_components().iter(),
    );

    let blob_row_id = results.component_row_id(&AssetVideo::descriptor_blob())?;
    let blob = results.component_instance::<Blob>(0, &AssetVideo::descriptor_blob())?;
    let media_type =
        results.component_instance::<MediaType>(0, &AssetVideo::descriptor_media_type());

    let video = ctx.store_context.caches.entry(|c: &mut VideoAssetCache| {
        let debug_name = entity_path.to_string();
        c.entry(
            debug_name,
            blob_row_id,
            &AssetVideo::descriptor_blob(),
            &blob,
            media_type.as_ref(),
            ctx.app_options().video_decoder_settings(),
        )
    });
    Some((video, blob))
}

impl TypedComponentFallbackProvider<components::EntityPath> for VideoFrameReferenceVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> components::EntityPath {
        ctx.target_entity_path.to_string().into()
    }
}

impl TypedComponentFallbackProvider<components::DrawOrder> for VideoFrameReferenceVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> components::DrawOrder {
        components::DrawOrder::DEFAULT_VIDEO
    }
}

re_viewer_context::impl_component_fallback_provider!(VideoFrameReferenceVisualizer => [components::EntityPath, components::DrawOrder]);
