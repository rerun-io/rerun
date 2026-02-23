mod video_frame_reference;
mod video_stream;

use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, EntityPathHash};
use re_renderer::renderer;
use re_renderer::resource_managers::ImageDataDesc;
use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_video::player::VideoPlaybackIssueSeverity;
use re_viewer_context::{ViewClass as _, ViewContext, ViewId, ViewSystemIdentifier};
pub use video_frame_reference::VideoFrameReferenceVisualizer;
pub use video_stream::VideoStreamVisualizer;

use super::{LoadingIndicator, SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};
use crate::{PickableRectSourceData, PickableTexturedRect, SpatialView2D};

/// Identify a video stream for a given video.
fn video_stream_id(
    entity_path: &EntityPath,
    view_id: ViewId,
    visualizer_name: ViewSystemIdentifier,
) -> re_video::player::VideoPlayerStreamId {
    re_video::player::VideoPlayerStreamId(
        re_log_types::hash::Hash64::hash((entity_path.hash(), view_id, visualizer_name)).hash64(),
    )
}

#[expect(clippy::too_many_arguments)]
fn visualize_video_frame_texture(
    ctx: &ViewContext<'_>,
    visualizer_data: &mut SpatialViewVisualizerData,
    video_frame_texture: re_renderer::video::VideoFrameTexture,
    entity_path: &EntityPath,
    depth_offset: re_renderer::DepthOffset,
    world_from_entity: glam::Affine3A,
    highlight: &re_viewer_context::ViewOutlineMasks,
    fallback_video_size: glam::Vec2,
    multiplicative_tint: egui::Rgba,
) {
    let re_renderer::video::VideoFrameTexture {
        texture,
        decoder_delay_state,
        show_loading_indicator,
        frame_info: _,
        source_pixel_format: _,
    } = video_frame_texture;

    let video_size = if let Some(texture) = &texture {
        glam::vec2(texture.width() as _, texture.height() as _)
    } else {
        fallback_video_size
    };

    // Make sure to use the video instead of texture size here,
    // since the texture may be a placeholder which doesn't have the full size yet.
    let top_left_corner_position = world_from_entity.transform_point3(glam::Vec3::ZERO);
    let extent_u = world_from_entity.transform_vector3(glam::Vec3::X * video_size.x);
    let extent_v = world_from_entity.transform_vector3(glam::Vec3::Y * video_size.y);

    if decoder_delay_state.should_request_more_frames() {
        // Keep polling for a fresh texture
        ctx.egui_ctx().request_repaint();
    }

    if show_loading_indicator {
        // Show loading rectangle:
        visualizer_data.loading_indicators.push(LoadingIndicator {
            center: top_left_corner_position + 0.5 * (extent_u + extent_v),
            half_extent_u: 0.5 * extent_u,
            half_extent_v: 0.5 * extent_v,
            reason: format!("Decoder: {decoder_delay_state:?}"),
        });
    }

    if let Some(texture) = texture {
        let textured_rect = renderer::TexturedRect {
            top_left_corner_position,
            extent_u,
            extent_v,
            colormapped_texture: renderer::ColormappedTexture::from_unorm_rgba(texture),
            options: renderer::RectangleOptions {
                texture_filter_magnification: renderer::TextureFilterMag::Nearest,
                texture_filter_minification: renderer::TextureFilterMin::Linear,
                outline_mask: highlight.overall,
                depth_offset,
                multiplicative_tint,
            },
        };
        visualizer_data.add_pickable_rect(
            PickableTexturedRect {
                ent_path: entity_path.clone(),
                textured_rect,
                source_data: PickableRectSourceData::Video,
            },
            ctx.view_class_identifier,
        );
    } else {
        // If we don't have a texture, still expand the bounding box,
        // so the default extents of the view show the loading indicator in the same place as if we had a texture.
        register_video_bounds_with_bounding_box(
            entity_path.hash(),
            visualizer_data,
            world_from_entity,
            video_size,
            ctx.view_class_identifier,
        );
    }
}

#[expect(clippy::too_many_arguments)]
fn show_video_playback_issue(
    ctx: &ViewContext<'_>,
    visualizer_data: &mut SpatialViewVisualizerData,
    highlight: &re_viewer_context::ViewOutlineMasks,
    world_from_entity: glam::Affine3A,
    error_string: String,
    severity: VideoPlaybackIssueSeverity,
    video_size: glam::Vec2,
    entity_path: &EntityPath,
    visualizer_instruction: VisualizerInstructionId,
) {
    // Register the full video bounds regardless for more stable default view extents for when the error
    // goes in and out of existence.
    register_video_bounds_with_bounding_box(
        entity_path.hash(),
        visualizer_data,
        world_from_entity,
        video_size,
        ctx.view_class_identifier,
    );

    let style = match severity {
        VideoPlaybackIssueSeverity::Error => UiLabelStyle::Error,
        VideoPlaybackIssueSeverity::Informational => UiLabelStyle::Default,
        VideoPlaybackIssueSeverity::Loading => {
            // Make sure to use the video instead of texture size here,
            // since the texture may be a placeholder which doesn't have the full size yet.
            let top_left_corner_position = world_from_entity.transform_point3(glam::Vec3::ZERO);
            let extent_u = world_from_entity.transform_vector3(glam::Vec3::X * video_size.x);
            let extent_v = world_from_entity.transform_vector3(glam::Vec3::Y * video_size.y);

            visualizer_data.loading_indicators.push(LoadingIndicator {
                center: top_left_corner_position + 0.5 * (extent_u + extent_v),
                half_extent_u: 0.5 * extent_u,
                half_extent_v: 0.5 * extent_v,
                reason: error_string,
            });
            return;
        }
    };

    let render_ctx = ctx.viewer_ctx.render_ctx();

    let video_error_image = match re_ui::icons::VIDEO_ERROR
        .load_image(ctx.viewer_ctx.egui_ctx(), egui::SizeHint::default())
    {
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
                    alpha_channel_usage: re_renderer::AlphaChannelUsage::AlphaChannelInUse,
                })
            },
        );

    let Ok(video_error_texture) = video_error_texture_result.inspect_err(|err| {
        re_log::error_once!("Failed to show video error icon: {err}");
    }) else {
        return; // We failed at failing…
    };

    let video_error_rect_size = {
        // Show the error icon with 2 texel per scene unit by default.
        let mut rect_size = glam::vec2(
            video_error_texture.width() as f32,
            video_error_texture.height() as f32,
        ) / 2.0;

        // But never larger than the area the video would take up.
        // (If we have to go smaller because of that, preserve the aspect ratio.)
        if rect_size.x > video_size.x {
            let scale = video_size.x / rect_size.x;
            rect_size *= scale;
        }
        if rect_size.y > video_size.y {
            let scale = video_size.y / rect_size.y;
            rect_size *= scale;
        }

        rect_size
    };

    // Center the icon in the middle of the video rectangle.
    // Don't ignore translation - if the user moved the video frame, we move the error message along.
    // But do ignore any rotation/scale on this, gets complicated to center and weird generally.
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

    visualizer_data.ui_labels.push(UiLabel {
        text: error_string,
        style,
        target: UiLabelTarget::Rect(label_target_rect),
        labeled_instance: re_entity_db::InstancePathHash::entity_all(entity_path),
        visualizer_instruction,
    });

    let error_rect = renderer::TexturedRect {
        top_left_corner_position: top_left_corner_position.extend(0.0),
        extent_u: glam::Vec3::X * video_error_rect_size.x,
        extent_v: glam::Vec3::Y * video_error_rect_size.y,
        colormapped_texture: renderer::ColormappedTexture::from_unorm_rgba(video_error_texture),
        options: renderer::RectangleOptions {
                texture_filter_magnification: renderer::TextureFilterMag::Linear,
                texture_filter_minification: renderer::TextureFilterMin::Linear,
                outline_mask: highlight.overall,
                #[expect(clippy::disallowed_methods)] // Ok to just dim it
                multiplicative_tint: egui::Rgba::from_gray(0.5),
                ..Default::default()
            },
    };

    visualizer_data.add_pickable_rect(
        PickableTexturedRect {
            ent_path: entity_path.clone(),
            textured_rect: error_rect,
            source_data: PickableRectSourceData::Placeholder,
        },
        ctx.view_class_identifier,
    );
}

fn register_video_bounds_with_bounding_box(
    entity_path: EntityPathHash,
    visualizer_data: &mut SpatialViewVisualizerData,
    world_from_entity: glam::Affine3A,
    video_size: glam::Vec2,
    class_identifier: ViewClassIdentifier,
) {
    // Only update the bounding box if this is a 2D view.
    // This is avoids a cyclic relationship where the image plane grows
    // the bounds which in turn influence the size of the image plane.
    // See: https://github.com/rerun-io/rerun/issues/3728
    if class_identifier != SpatialView2D::identifier() {
        return;
    }

    let top_left = glam::Vec3::from(world_from_entity.translation);

    visualizer_data.add_bounding_box(
        entity_path,
        macaw::BoundingBox {
            min: top_left,
            max: top_left + glam::Vec3::new(video_size.x, video_size.y, 0.0),
        },
        world_from_entity,
    );
}
