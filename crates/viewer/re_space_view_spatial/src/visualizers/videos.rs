use std::sync::Arc;

use re_log_types::{hash::Hash64, EntityPath};
use re_renderer::{
    external::re_video::VideoLoadError,
    renderer::{
        ColormappedTexture, RectangleOptions, TextureFilterMag, TextureFilterMin, TexturedRect,
    },
    resource_managers::ImageDataDesc,
    video::{Video, VideoFrameTexture},
};
use re_types::{
    archetypes::{AssetVideo, VideoFrameReference},
    components::{Blob, MediaType, VideoTimestamp},
    Archetype, Loggable as _,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewClass as _, SpaceViewId,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, VideoCache, ViewContext,
    ViewContextCollection, ViewQuery, ViewerContext, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::{entity_iterator, filter_visualizable_2d_entities, LoadingSpinner},
    PickableRectSourceData, PickableTexturedRect, SpatialSpaceView2D,
};

use super::{
    entity_iterator::process_archetype, SpatialViewVisualizerData, UiLabel, UiLabelTarget,
};

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
                    results.iter_as(timeline, re_types::components::EntityPath::name());

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

                    self.process_video_frame(
                        ctx,
                        spatial_ctx,
                        video_timestamp,
                        video_references,
                        entity_path,
                        view_query.space_view_id,
                    );
                }

                Ok(())
            },
        )?;

        Ok(vec![PickableTexturedRect::to_draw_data(
            render_ctx,
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
        view_id: SpaceViewId,
    ) {
        re_tracing::profile_function!();

        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return;
        };

        let decode_stream_id = re_renderer::video::VideoDecodingStreamId(
            Hash64::hash((entity_path.hash(), view_id)).hash64(),
        );

        // Follow the reference to the video asset.
        let video_reference: EntityPath = video_references
            .and_then(|v| v.first().map(|e| e.as_str().into()))
            .unwrap_or_else(|| self.fallback_for(ctx).as_str().into());
        let query_result = latest_at_query_video_from_datastore(ctx.viewer_ctx, &video_reference);

        let world_from_entity = spatial_ctx
            .transform_info
            .single_entity_transform_required(ctx.target_entity_path, Self::identifier().as_str());

        // Note that we may or may not know the video size independently of error occurrence.
        // (if it's just a decoding error we may still know the size from the container!)
        // In case we haven error we want to center the message in the middle, so we need some area.
        // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
        let mut video_resolution = glam::vec2(1280.0, 720.0);

        match query_result {
            None => {
                self.show_video_error(
                    ctx,
                    spatial_ctx,
                    world_from_entity,
                    format!("No video asset at {video_reference:?}"),
                    video_resolution,
                    entity_path,
                );
            }

            Some((video, video_data)) => match video.as_ref() {
                Ok(video) => {
                    video_resolution = glam::vec2(video.width() as _, video.height() as _);

                    match video.frame_at(
                        render_ctx,
                        decode_stream_id,
                        video_timestamp.as_seconds(),
                        video_data.as_slice(),
                    ) {
                        Ok(VideoFrameTexture {
                            texture,
                            time_range: _, // TODO(emilk): maybe add to `PickableTexturedRect` and `PickingHitType::TexturedRect` so we can show on hover?
                            is_pending,
                            show_spinner,
                        }) => {
                            // Make sure to use the video instead of texture size here,
                            // since the texture may be a placeholder which doesn't have the full size yet.
                            let top_left_corner_position =
                                world_from_entity.transform_point3(glam::Vec3::ZERO);
                            let extent_u = world_from_entity
                                .transform_vector3(glam::Vec3::X * video_resolution.x);
                            let extent_v = world_from_entity
                                .transform_vector3(glam::Vec3::Y * video_resolution.y);

                            if is_pending {
                                // Keep polling for a fresh texture
                                ctx.viewer_ctx.egui_ctx.request_repaint();
                            }

                            if show_spinner {
                                // Show loading rectangle:
                                self.data.loading_spinners.push(LoadingSpinner {
                                    center: top_left_corner_position + 0.5 * (extent_u + extent_v),
                                    half_extent_u: 0.5 * extent_u,
                                    half_extent_v: 0.5 * extent_v,
                                });
                            }

                            let textured_rect = TexturedRect {
                                top_left_corner_position,
                                extent_u,
                                extent_v,
                                colormapped_texture: ColormappedTexture::from_unorm_rgba(texture),
                                options: RectangleOptions {
                                    texture_filter_magnification: TextureFilterMag::Nearest,
                                    texture_filter_minification: TextureFilterMin::Linear,
                                    outline_mask: spatial_ctx.highlight.overall,
                                    ..Default::default()
                                },
                            };
                            self.data.pickable_rects.push(PickableTexturedRect {
                                ent_path: entity_path.clone(),
                                textured_rect,
                                source_data: PickableRectSourceData::Video,
                            });
                        }

                        Err(err) => {
                            self.show_video_error(
                                ctx,
                                spatial_ctx,
                                world_from_entity,
                                err.to_string(),
                                video_resolution,
                                entity_path,
                            );
                        }
                    }
                }
                Err(err) => {
                    self.show_video_error(
                        ctx,
                        spatial_ctx,
                        world_from_entity,
                        err.to_string(),
                        video_resolution,
                        entity_path,
                    );
                }
            },
        }

        if spatial_ctx.space_view_class_identifier == SpatialSpaceView2D::identifier() {
            let bounding_box = re_math::BoundingBox::from_min_size(
                world_from_entity.transform_point3(glam::Vec3::ZERO),
                video_resolution.extend(0.0),
            );
            self.data
                .add_bounding_box(entity_path.hash(), bounding_box, world_from_entity);
        }
    }

    fn show_video_error(
        &mut self,
        ctx: &re_viewer_context::QueryContext<'_>,
        spatial_ctx: &SpatialSceneEntityContext<'_>,
        world_from_entity: glam::Affine3A,
        error_string: String,
        video_size: glam::Vec2,
        entity_path: &EntityPath,
    ) {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return;
        };

        let video_error_texture_result = render_ctx
            .texture_manager_2d
            .get_or_try_create_with::<image::ImageError>(
                Hash64::hash("video_error").hash64(),
                render_ctx,
                || {
                    let mut reader = image::io::Reader::new(std::io::Cursor::new(
                        re_ui::icons::VIDEO_ERROR.png_bytes,
                    ));
                    reader.set_format(image::ImageFormat::Png);
                    let dynamic_image = reader.decode()?;

                    Ok(ImageDataDesc {
                        label: "video_error".into(),
                        data: std::borrow::Cow::Owned(dynamic_image.to_rgba8().to_vec()),
                        format: re_renderer::external::wgpu::TextureFormat::Rgba8UnormSrgb.into(),
                        width_height: [dynamic_image.width(), dynamic_image.height()],
                    })
                },
            );

        let Ok(video_error_texture) = video_error_texture_result.inspect_err(|err| {
            re_log::error_once!("Failed to show video error icon: {err}");
        }) else {
            return; // We failed at failing…
        };

        // Center the icon in the middle of the video rectangle.
        // Don't ignore translation - if the user moved the video frame, we move the error message along.
        // But do ignore any rotation/scale on this, gets complicated to center and weird generally.
        let mut video_error_rect_size = glam::vec2(
            video_error_texture.width() as _,
            video_error_texture.height() as _,
        );
        // If we're in a 2D view, make the error rect take a fixed amount of view space.
        // This makes it look a lot nicer for very small & very large videos.
        if let Some(state) = ctx
            .view_state
            .as_any()
            .downcast_ref::<SpatialSpaceViewState>()
        {
            if let Some(bounds) = state.visual_bounds_2d {
                // Aim for 1/8 of the larger visual bounds axis.
                let max_extent = bounds.x_range.abs_len().max(bounds.y_range.abs_len()) as f32;
                if max_extent > 0.0 {
                    let video_error_rect_aspect = video_error_rect_size.x / video_error_rect_size.y;
                    let extent_x = max_extent / 8.0;
                    let extent_y = extent_x / video_error_rect_aspect;
                    video_error_rect_size = glam::vec2(extent_x, extent_y);
                }
            }
        }

        let center = glam::Vec3::from(world_from_entity.translation).truncate() + video_size * 0.5;
        let top_left_corner_position = center - video_error_rect_size * 0.5;

        // Add a label that annotates a rectangle that is a bit bigger than the error icon.
        // This makes the label track the icon better than putting it at a point.
        let label_target_rect = egui::Rect::from_min_size(
            egui::pos2(
                top_left_corner_position.x - video_error_rect_size.x,
                top_left_corner_position.y,
            ),
            egui::vec2(video_error_rect_size.x * 3.0, video_error_rect_size.y),
        );
        self.data.ui_labels.push(UiLabel {
            text: error_string,
            color: egui::Color32::LIGHT_RED,
            target: UiLabelTarget::Rect(label_target_rect),
            labeled_instance: re_entity_db::InstancePathHash::entity_all(entity_path),
        });

        let error_rect = TexturedRect {
            top_left_corner_position: top_left_corner_position.extend(0.0),
            extent_u: glam::Vec3::X * video_error_rect_size.x,
            extent_v: glam::Vec3::Y * video_error_rect_size.y,
            colormapped_texture: ColormappedTexture::from_unorm_rgba(video_error_texture),
            options: RectangleOptions {
                texture_filter_magnification: TextureFilterMag::Linear,
                texture_filter_minification: TextureFilterMin::Linear,
                outline_mask: spatial_ctx.highlight.overall,
                ..Default::default()
            },
        };

        self.data.pickable_rects.push(PickableTexturedRect {
            ent_path: entity_path.clone(),
            textured_rect: error_rect,
            source_data: PickableRectSourceData::ErrorPlaceholder,
        });
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

    let results = ctx.recording().query_caches().latest_at(
        ctx.recording_store(),
        &query,
        entity_path,
        AssetVideo::all_components().iter().copied(),
    );

    let blob_row_id = results.component_row_id(&Blob::name())?;
    let blob = results.component_instance::<Blob>(0)?;
    let media_type = results.component_instance::<MediaType>(0);

    let video = ctx.cache.entry(|c: &mut VideoCache| {
        let debug_name = entity_path.to_string();
        c.entry(
            debug_name,
            blob_row_id,
            &blob,
            media_type.as_ref(),
            ctx.app_options.video_decoder_hw_acceleration,
        )
    });
    Some((video, blob))
}

impl TypedComponentFallbackProvider<re_types::components::EntityPath>
    for VideoFrameReferenceVisualizer
{
    fn fallback_for(
        &self,
        ctx: &re_viewer_context::QueryContext<'_>,
    ) -> re_types::components::EntityPath {
        ctx.target_entity_path.to_string().into()
    }
}

re_viewer_context::impl_component_fallback_provider!(VideoFrameReferenceVisualizer => [re_types::components::EntityPath]);
