mod video_frame_reference;
mod video_stream;

use re_types::ViewClassIdentifier;
pub use video_frame_reference::VideoFrameReferenceVisualizer;
pub use video_stream::VideoStreamVisualizer;

use re_log_types::{EntityPath, EntityPathHash, hash::Hash64};
use re_renderer::{renderer, resource_managers::ImageDataDesc};
use re_viewer_context::{ViewContext, ViewId, ViewSystemIdentifier};

use crate::{PickableRectSourceData, PickableTexturedRect, SpatialViewState};

use super::{LoadingSpinner, SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};

/// Identify a video stream for a given video.
fn video_stream_id(
    entity_path: &EntityPath,
    view_id: ViewId,
    visualizer_name: ViewSystemIdentifier,
) -> re_renderer::video::VideoPlayerStreamId {
    re_renderer::video::VideoPlayerStreamId(
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
) {
    let re_renderer::video::VideoFrameTexture {
        texture,
        is_pending,
        show_spinner,
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

    if is_pending {
        // Keep polling for a fresh texture
        ctx.egui_ctx().request_repaint();
    }

    if show_spinner {
        // Show loading rectangle:
        visualizer_data.loading_spinners.push(LoadingSpinner {
            center: top_left_corner_position + 0.5 * (extent_u + extent_v),
            half_extent_u: 0.5 * extent_u,
            half_extent_v: 0.5 * extent_v,
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
                ..Default::default()
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
        // so the default extents of the view show the spinner in the same place as if we had a texture.
        register_video_bounds_with_bounding_box(
            entity_path.hash(),
            visualizer_data,
            world_from_entity,
            video_size,
            ctx.view_class_identifier,
        );
    }
}

fn show_video_error(
    ctx: &ViewContext<'_>,
    visualizer_data: &mut SpatialViewVisualizerData,
    highlight: &re_viewer_context::ViewOutlineMasks,
    world_from_entity: glam::Affine3A,
    error_string: String,
    video_size: glam::Vec2,
    entity_path: &EntityPath,
) {
    // Register the full video bounds regardless for more stable default view extents for when the error
    // goes in and out of existance.
    // The size of the error icon depends on the bounds in turn, making this extra important!
    register_video_bounds_with_bounding_box(
        entity_path.hash(),
        visualizer_data,
        world_from_entity,
        video_size,
        ctx.view_class_identifier,
    );

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
    if let Some(state) = ctx.view_state.as_any().downcast_ref::<SpatialViewState>() {
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
    visualizer_data.ui_labels.push(UiLabel {
        text: error_string,
        style: UiLabelStyle::Error,
        target: UiLabelTarget::Rect(label_target_rect),
        labeled_instance: re_entity_db::InstancePathHash::entity_all(entity_path),
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
            source_data: PickableRectSourceData::ErrorPlaceholder,
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

    // To avoid circular dependency of the bounds and the error rectangle (causing flickering),
    // make sure the bounding box contains the entire (speculative) video rectangle.
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
