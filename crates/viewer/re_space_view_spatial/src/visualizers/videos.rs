use egui::mutex::Mutex;

use re_log_types::EntityPath;
use re_renderer::{
    renderer::{
        ColormappedTexture, RectangleOptions, TextureFilterMag, TextureFilterMin, TexturedRect,
    },
    video::{FrameDecodingResult, Video},
};
use re_types::{
    archetypes::{AssetVideo, VideoFrameReference},
    components::{Blob, EntityPath as EntityPathReferenceComponent, MediaType, VideoTimestamp},
    datatypes::VideoTimeMode,
    Archetype, Loggable as _,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewClass as _, SpaceViewSystemExecutionError,
    ViewContext, ViewContextCollection, ViewQuery, ViewerContext, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    video_cache::{VideoCache, VideoCacheKey},
    view_kind::SpatialSpaceViewKind,
    visualizers::{entity_iterator, filter_visualizable_2d_entities},
    SpatialSpaceView2D,
};

use super::bounding_box_for_textured_rect;
use super::{entity_iterator::process_archetype, SpatialViewVisualizerData};

pub struct VideoFrameReferenceVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for VideoFrameReferenceVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
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
        entities: ApplicableEntities,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut rectangles = Vec::new();

        process_archetype::<Self, VideoFrameReference, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                // TODO(andreas): Should ignore range queries here and only do latest-at.
                // Not only would this simplify the code here quite a bit, it would also avoid lots of overhead.
                // Same is true for the image visualizers in general - there seems to be no practical reason to do range queries
                // for visualization here.
                use re_space_view::RangeResultsExt as _;

                let timeline = ctx.query.timeline();
                let entity_path = ctx.target_entity_path;

                let Some(all_video_timestamp_chunks) =
                    results.get_required_chunks(&VideoTimestamp::name())
                else {
                    return Ok(());
                };
                let all_video_references =
                    results.iter_as(timeline, EntityPathReferenceComponent::name());

                for (_index, video_timestamps, video_references) in re_query::range_zip_1x1(
                    entity_iterator::iter_component(
                        &all_video_timestamp_chunks,
                        timeline,
                        VideoTimestamp::name(),
                    ),
                    all_video_references.string(),
                ) {
                    let Some(video_timestamp): Option<&VideoTimestamp> = video_timestamps.first()
                    else {
                        continue;
                    };

                    // Follow the reference to the video asset.
                    let video_reference = video_references
                        .and_then(|v| v.first().map(|e| e.as_str().into()))
                        .unwrap_or_else(|| entity_path.clone());
                    let Some(video) =
                        latest_at_query_video_from_datastore(ctx.viewer_ctx, &video_reference)
                    else {
                        continue;
                    };

                    let timestamp_in_seconds = match video_timestamp.time_mode {
                        VideoTimeMode::Nanoseconds => video_timestamp.video_time as f64 / 1e9,
                    };

                    let (texture_result, video_width, video_height) = {
                        let mut video = video.lock(); // TODO(andreas): Interior mutability for re_renderer's video would be nice.
                        (
                            video.frame_at(timestamp_in_seconds),
                            video.width(),
                            video.height(),
                        )
                    };

                    let texture = match texture_result {
                        FrameDecodingResult::Ready(texture) => texture,
                        FrameDecodingResult::Pending(texture) => {
                            ctx.viewer_ctx.egui_ctx.request_repaint();
                            texture
                        }
                        FrameDecodingResult::Error(err) => {
                            // TODO(#7373): show this error in the ui
                            re_log::error_once!(
                                "Failed to decode video frame for {entity_path}: {err}"
                            );
                            continue;
                        }
                    };

                    let world_from_entity =
                        spatial_ctx.transform_info.single_entity_transform_required(
                            ctx.target_entity_path,
                            Self::identifier().as_str(),
                        );
                    let textured_rect = textured_rect_for_video_frame(
                        world_from_entity,
                        video_width,
                        video_height,
                        texture,
                    );

                    if spatial_ctx.space_view_class_identifier == SpatialSpaceView2D::identifier() {
                        // Only update the bounding box if this is a 2D space view.
                        // This is avoids a cyclic relationship where the image plane grows
                        // the bounds which in turn influence the size of the image plane.
                        // See: https://github.com/rerun-io/rerun/issues/3728
                        self.data.add_bounding_box(
                            entity_path.hash(),
                            bounding_box_for_textured_rect(&textured_rect),
                            world_from_entity,
                        );
                    }

                    rectangles.push(textured_rect);
                }

                Ok(())
            },
        )?;

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!("Failed to create rectangle draw data from images: {err}");
            }
        }

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

fn textured_rect_for_video_frame(
    world_from_entity: glam::Affine3A,
    video_width: u32,
    video_height: u32,
    texture: re_renderer::resource_managers::GpuTexture2D,
) -> TexturedRect {
    TexturedRect {
        top_left_corner_position: world_from_entity.transform_point3(glam::Vec3::ZERO),
        // Make sure to use the video instead of texture size here,
        // since it may be a placeholder which doesn't have the full size yet.
        extent_u: world_from_entity.transform_vector3(glam::Vec3::X * video_width as f32),
        extent_v: world_from_entity.transform_vector3(glam::Vec3::Y * video_height as f32),

        colormapped_texture: ColormappedTexture::from_unorm_rgba(texture),
        options: RectangleOptions {
            texture_filter_magnification: TextureFilterMag::Nearest,
            texture_filter_minification: TextureFilterMin::Linear,
            ..Default::default()
        },
    }
}

/// Queries a video from the datstore and caches it in the video cache.
///
/// Note that this does *NOT* check the blueprint store at all.
/// For this, we'd need a [`re_viewer_context::DataResult`] instead of merely a [`EntityPath`].
fn latest_at_query_video_from_datastore(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
) -> Option<std::sync::Arc<Mutex<Video>>> {
    let query = ctx.current_query();

    let results = ctx.recording().query_caches().latest_at(
        ctx.recording_store(),
        &query,
        entity_path,
        AssetVideo::all_components().iter().copied(),
    );

    let blob_row_id = results.component_row_id(&Blob::name())?;
    let blob = results.component_instance::<Blob>(0)?;
    let media_type = results.component_instance::<MediaType>(0);

    ctx.cache.entry(|c: &mut VideoCache| {
        c.entry(
            &entity_path.to_string(),
            VideoCacheKey {
                versioned_instance_path_hash: re_entity_db::InstancePathHash::entity_all(
                    entity_path,
                )
                .versioned(blob_row_id),
                media_type: media_type.clone(),
            },
            &blob,
            media_type,
            ctx.render_ctx?,
        )
    })
}

re_viewer_context::impl_component_fallback_provider!(VideoFrameReferenceVisualizer => []);
