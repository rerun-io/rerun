mod video_frame_reference;
mod video_stream;

pub use video_frame_reference::VideoFrameReferenceVisualizer;
pub use video_stream::VideoStreamVisualizer;

use re_log_types::EntityPath;
use re_renderer::renderer;
use re_viewer_context::{ViewId, ViewSystemIdentifier, ViewerContext};

use crate::{PickableRectSourceData, PickableTexturedRect};

use super::{LoadingSpinner, SpatialViewVisualizerData};

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

fn visualize_video_frame_texture(
    ctx: &ViewerContext<'_>,
    visualizer_data: &mut SpatialViewVisualizerData,
    video_frame_texture: re_renderer::video::VideoFrameTexture,
    entity_path: &EntityPath,
    depth_offset: re_renderer::DepthOffset,
    world_from_entity: glam::Affine3A,
    highlight: &re_viewer_context::ViewOutlineMasks,
    video_resolution: &mut glam::Vec2,
) {
    let re_renderer::video::VideoFrameTexture {
        texture,
        is_pending,
        show_spinner,
        frame_info: _,
        source_pixel_format: _,
    } = video_frame_texture;

    if let Some(texture) = &texture {
        *video_resolution = glam::vec2(texture.width() as _, texture.height() as _);
    }

    // Make sure to use the video instead of texture size here,
    // since the texture may be a placeholder which doesn't have the full size yet.
    let top_left_corner_position = world_from_entity.transform_point3(glam::Vec3::ZERO);
    let extent_u = world_from_entity.transform_vector3(glam::Vec3::X * video_resolution.x);
    let extent_v = world_from_entity.transform_vector3(glam::Vec3::Y * video_resolution.y);

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
        visualizer_data.pickable_rects.push(PickableTexturedRect {
            ent_path: entity_path.clone(),
            textured_rect,
            source_data: PickableRectSourceData::Video,
        });
    }
}
