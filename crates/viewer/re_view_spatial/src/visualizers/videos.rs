use std::sync::Arc;

use re_log_types::{EntityPath, hash::Hash64};
use re_renderer::{
    external::re_video::VideoLoadError,
    renderer::{
        ColormappedTexture, RectangleOptions, TextureFilterMag, TextureFilterMin, TexturedRect,
    },
    resource_managers::ImageDataDesc,
    video::{Video, VideoFrameTexture},
};
use re_types::{
    Archetype as _,
    archetypes::{AssetVideo, VideoFrameReference},
    components::{self, Blob, MediaType, VideoTimestamp},
};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, TypedComponentFallbackProvider, VideoCache,
    ViewClass as _, ViewContext, ViewContextCollection, ViewId, ViewQuery,
    ViewSystemExecutionError, ViewerContext, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    PickableRectSourceData, PickableTexturedRect, SpatialView2D,
    contexts::SpatialSceneEntityContext,
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::{LoadingSpinner, entity_iterator, filter_visualizable_2d_entities},
};

use super::{
    SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget,
    entity_iterator::process_archetype,
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

        let player_stream_id = re_renderer::video::VideoPlayerStreamId(
            Hash64::hash((entity_path.hash(), view_id)).hash64(),
        );

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
                    if let Some(coded_dimensions) = video.data().coded_dimensions {
                        video_resolution =
                            glam::vec2(coded_dimensions[0] as _, coded_dimensions[1] as _);
                    }

                    match video.frame_at(
                        ctx.render_ctx(),
                        player_stream_id,
                        video_timestamp.as_secs(),
                        &std::iter::once(video_data.as_ref()).collect(),
                    ) {
                        Ok(VideoFrameTexture {
                            texture,
                            is_pending,
                            show_spinner,
                            frame_info: _, // TODO(emilk): maybe add to `PickableTexturedRect` and `PickingHitType::TexturedRect` so we can show on hover?
                            source_pixel_format: _,
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
                                ctx.egui_ctx().request_repaint();
                            }

                            if show_spinner {
                                // Show loading rectangle:
                                self.data.loading_spinners.push(LoadingSpinner {
                                    center: top_left_corner_position + 0.5 * (extent_u + extent_v),
                                    half_extent_u: 0.5 * extent_u,
                                    half_extent_v: 0.5 * extent_v,
                                });
                            }

                            if let Some(texture) = texture {
                                let textured_rect = TexturedRect {
                                    top_left_corner_position,
                                    extent_u,
                                    extent_v,
                                    colormapped_texture: ColormappedTexture::from_unorm_rgba(
                                        texture,
                                    ),
                                    options: RectangleOptions {
                                        texture_filter_magnification: TextureFilterMag::Nearest,
                                        texture_filter_minification: TextureFilterMin::Linear,
                                        outline_mask: spatial_ctx.highlight.overall,
                                        depth_offset: spatial_ctx.depth_offset,
                                        ..Default::default()
                                    },
                                };
                                self.data.pickable_rects.push(PickableTexturedRect {
                                    ent_path: entity_path.clone(),
                                    textured_rect,
                                    source_data: PickableRectSourceData::Video,
                                });
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

        if spatial_ctx.view_class_identifier == SpatialView2D::identifier() {
            let bounding_box = macaw::BoundingBox::from_min_size(
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
        let render_ctx = ctx.render_ctx();

        let video_error_image =
            match re_ui::icons::VIDEO_ERROR.load_image(ctx.egui_ctx(), egui::SizeHint::default()) {
                Err(err) => {
                    re_log::error_once!("Failed to load video error icon: {err}");
                    return;
                }
                Ok(egui::load::ImagePoll::Ready { image }) => image,
                Ok(egui::load::ImagePoll::Pending { .. }) => {
                    return; // wait for it to load
                }
            };

        let video_error_texture_result = render_ctx
            .texture_manager_2d
            .get_or_try_create_with::<image::ImageError>(
                Hash64::hash("video_error").hash64(),
                render_ctx,
                || {
                    Ok(ImageDataDesc {
                        label: "video_error".into(),
                        data: std::borrow::Cow::Owned(video_error_image.as_raw().to_vec()),
                        format: re_renderer::external::wgpu::TextureFormat::Rgba8UnormSrgb.into(),
                        width_height: [
                            video_error_image.width() as _,
                            video_error_image.height() as _,
                        ],
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
        if let Some(state) = ctx.view_state().as_any().downcast_ref::<SpatialViewState>() {
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
            style: UiLabelStyle::Error,
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
                #[expect(clippy::disallowed_methods)] // Ok to just dim it
                multiplicative_tint: egui::Rgba::from_gray(0.5),
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

    let results = ctx.recording_engine().cache().latest_at(
        &query,
        entity_path,
        AssetVideo::all_components().iter(),
    );

    let blob_row_id = results.component_row_id(&AssetVideo::descriptor_blob())?;
    let blob = results.component_instance::<Blob>(0, &AssetVideo::descriptor_blob())?;
    let media_type =
        results.component_instance::<MediaType>(0, &AssetVideo::descriptor_media_type());

    let video = ctx.store_context.caches.entry(|c: &mut VideoCache| {
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
        components::DrawOrder::DEFAULT_VIDEO_FRAME_REFERENCE
    }
}

re_viewer_context::impl_component_fallback_provider!(VideoFrameReferenceVisualizer => [components::EntityPath, components::DrawOrder]);
