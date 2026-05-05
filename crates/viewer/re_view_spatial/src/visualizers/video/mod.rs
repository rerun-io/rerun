mod encoded_depth_image;
mod encoded_image;
mod video_frame_reference;
mod video_stream;

pub use encoded_depth_image::{EncodedDepthImageVisualizer, EncodedDepthImageVisualizerOutput};
pub use encoded_image::EncodedImageVisualizer;
use nohash_hasher::IntMap;
use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, EntityPathHash};
use re_renderer::renderer;
use re_renderer::resource_managers::{GpuTexture2D, ImageDataDesc};
use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::components::Opacity;
use re_ui::ContextExt as _;
use re_video::player::{VideoPlaybackIssueSeverity, VideoPlayerError};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    VideoStreamCache, VideoStreamProcessingError, ViewClass as _, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    VisualizerExecutionOutput, typed_fallback_for, video_stream_time_from_query,
};
pub use video_frame_reference::VideoFrameReferenceVisualizer;
pub use video_stream::VideoStreamVisualizer;

use super::{LoadingIndicator, SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};
use crate::contexts::EntityDepthOffsets;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::DepthImageProcessResult;
use crate::visualizers::utilities::{
    spatial_view_kind_from_view_class, transform_info_for_archetype_or_report_error,
};
use crate::{PickableRectSourceData, PickableTexturedRect, SpatialView2D, TransformTreeContext};

type GetCodecFn = dyn Fn(
    &ViewContext<'_>,
    &re_chunk_store::LatestAtQuery,
    &re_viewer_context::DataResult,
    &re_viewer_context::VisualizerInstruction,
    &VisualizerExecutionOutput,
) -> Result<re_video::VideoCodec, VideoStreamProcessingError>;

/// Configuration for rendering depth textures with a colormap.
///
/// When provided to [`execute_video_stream_like`], the video frame texture
/// will be colormapped instead of treated as a regular RGBA image.
/// In 3D views with a pinhole camera, a depth cloud will be rendered instead.
pub(super) struct DepthTextureConfig {
    pub colormap: re_renderer::Colormap,
    pub range: [f32; 2],
    pub depth_meter: re_sdk_types::components::DepthMeter,
    pub fill_ratio: re_sdk_types::components::FillRatio,
}

impl DepthTextureConfig {
    fn to_colormapped_texture(&self, texture: GpuTexture2D) -> renderer::ColormappedTexture {
        renderer::ColormappedTexture {
            texture,
            range: self.range,
            decode_srgb: false,
            texture_alpha: renderer::TextureAlpha::Opaque,
            gamma: 1.0,
            color_mapper: renderer::ColorMapper::Function(self.colormap),
            shader_decoding: None,
        }
    }
}

/// Callback to produce per-entity depth texture configuration.
///
/// Called once per entity/instruction to query colormap, value range, etc.
pub(super) type GetDepthConfigFn = dyn Fn(
    &ViewContext<'_>,
    &re_chunk_store::LatestAtQuery,
    &re_viewer_context::DataResult,
    &re_viewer_context::VisualizerInstruction,
    &VisualizerExecutionOutput,
) -> DepthTextureConfig;

struct DepthHandler<'a> {
    get_depth_config: &'a GetDepthConfigFn,
    depth_cloud_entities: &'a mut IntMap<EntityPathHash, DepthImageProcessResult>,
}

/// Used to pass the required context to [`execute_video_stream_like`].
struct VideoStreamCtx<'a> {
    view_context: &'a ViewContext<'a>,
    view_query: &'a ViewQuery<'a>,
    context_systems: &'a ViewContextCollection,
    data: &'a mut SpatialViewVisualizerData,

    visualizer_name: ViewSystemIdentifier,

    archetype_name: re_sdk_types::ArchetypeName,
    sample_component: re_sdk_types::ComponentIdentifier,
    opacity_component: Option<re_sdk_types::ComponentIdentifier>,

    get_codec: &'a GetCodecFn,

    depth_handler: Option<DepthHandler<'a>>,
}

impl<'a> VideoStreamCtx<'a> {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        view_context: &'a ViewContext<'a>,
        view_query: &'a ViewQuery<'a>,
        context_systems: &'a ViewContextCollection,
        data: &'a mut SpatialViewVisualizerData,

        visualizer_name: ViewSystemIdentifier,

        archetype_name: re_sdk_types::ArchetypeName,
        sample_component: re_sdk_types::ComponentIdentifier,

        get_codec: &'a GetCodecFn,
    ) -> Self {
        Self {
            view_context,
            view_query,
            context_systems,
            data,
            visualizer_name,
            archetype_name,
            sample_component,
            opacity_component: None,
            get_codec,
            depth_handler: None,
        }
    }

    pub fn with_opacity_component(
        mut self,
        opacity_component: re_sdk_types::ComponentIdentifier,
    ) -> Self {
        self.opacity_component = Some(opacity_component);

        self
    }

    pub fn with_depth_handler(
        mut self,
        get_depth_config: &'a GetDepthConfigFn,
        depth_cloud_entities: &'a mut IntMap<EntityPathHash, DepthImageProcessResult>,
    ) -> Self {
        self.depth_handler = Some(DepthHandler {
            get_depth_config,
            depth_cloud_entities,
        });

        self
    }
}

impl<'a> std::ops::Deref for VideoStreamCtx<'a> {
    type Target = ViewContext<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.view_context
    }
}

fn execute_video_stream_like(
    mut ctx: VideoStreamCtx<'_>,
) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
    re_tracing::profile_function!();

    let output = VisualizerExecutionOutput::default();

    let viewer_ctx = ctx.viewer_ctx;
    let view_kind = spatial_view_kind_from_view_class(ctx.view_class_identifier);
    let transforms = ctx.context_systems.get::<TransformTreeContext>(&output)?;
    let depth_offsets = ctx.context_systems.get::<EntityDepthOffsets>(&output)?;
    let latest_at = ctx.view_query.latest_at_query();
    let mut depth_clouds = Vec::new();

    for (data_result, instruction) in ctx
        .view_query
        .iter_visualizer_instruction_for(ctx.visualizer_name)
    {
        let entity_path = &data_result.entity_path;

        re_tracing::profile_scope!("Entity", entity_path.to_string().as_str());

        let Some(transform_info) = transform_info_for_archetype_or_report_error(
            entity_path,
            transforms,
            Some(SpatialViewKind::TwoD),
            view_kind,
            &instruction.id,
            &output,
        ) else {
            continue;
        };

        let world_from_entity = transform_info
            .single_transform_required_for_entity(entity_path, ctx.archetype_name)
            .as_affine3a();
        let query_context = ctx.query_context(data_result, latest_at.clone(), instruction.id);
        let highlight = ctx
            .view_query
            .highlights
            .entity_outline_mask(entity_path.hash());

        // Note that we may or may not know the video size independently of error occurrence.
        // (if it's just a decoding error we may still know the size from the container!)
        // In case we haven error we want to center the message in the middle, so we need some area.
        // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
        let mut video_resolution = glam::vec2(1280.0, 720.0);

        let opacity = ctx.opacity_component.map(|opacity_component| {
            let results = data_result.latest_at_with_blueprint_resolved_data_for_component(
                &ctx,
                &latest_at,
                opacity_component,
                Some(instruction),
            );
            if results.any_missing_chunks() {
                output.set_missing_chunks();
            }
            results
                .get_mono::<Opacity>(opacity_component)
                .unwrap_or_else(|| {
                    typed_fallback_for(
                        &re_viewer_context::QueryContext {
                            view_ctx: &ctx,
                            target_entity_path: entity_path,
                            instruction_id: Some(instruction.id),
                            archetype_name: Some(ctx.archetype_name),
                            query: latest_at.clone(),
                        },
                        opacity_component,
                    )
                })
        });

        // Perform a latest-at query for the sample component to give the video stream cache something to hook onto.
        let _sample_result = data_result.latest_at_with_blueprint_resolved_data_for_component(
            &ctx,
            &latest_at,
            ctx.sample_component,
            Some(instruction),
        );

        let video = match viewer_ctx
            .store_context
            .memoizer(|c: &mut VideoStreamCache| {
                c.entry(
                    viewer_ctx.recording(),
                    entity_path,
                    ctx.view_query.timeline,
                    viewer_ctx.app_options().video_decoder_settings(),
                    ctx.sample_component,
                    &|| (ctx.get_codec)(&ctx, &latest_at, data_result, instruction, &output),
                )
            }) {
            Ok(video) => video,

            Err(err) => {
                let (description, severity) = match err {
                    VideoStreamProcessingError::NoVideoSamplesFound => (
                        format!("No video samples available for {entity_path:?}"),
                        VideoPlaybackIssueSeverity::Informational,
                    ),
                    VideoStreamProcessingError::UnloadedCodec => (
                        "Codec not loaded yet".to_owned(),
                        VideoPlaybackIssueSeverity::Loading,
                    ),
                    VideoStreamProcessingError::InvalidVideoSampleType(_)
                    | VideoStreamProcessingError::MissingCodec
                    | VideoStreamProcessingError::FailedReadingCodec(_)
                    | VideoStreamProcessingError::OutOfOrderSamples
                    | VideoStreamProcessingError::UnexpectedChunkChanges => (
                        format!("Failed to play video at {entity_path:?}: {err}"),
                        VideoPlaybackIssueSeverity::Error,
                    ),
                };

                show_video_frame(
                    ctx.view_context,
                    ctx.data,
                    entity_path,
                    world_from_entity,
                    highlight,
                    video_resolution,
                    instruction.id,
                    None,
                    Some(VideoPlaybackIssue::custom(description, severity)),
                    None,
                    None,
                );
                continue;
            }
        };

        let video_time = video_stream_time_from_query(&query_context.query);
        if video_time.0 < 0 {
            // The frame is from before the video starts, so nothing to draw here!
            continue;
        }

        let bit_depth = video
            .read()
            .video_renderer
            .data_descr()
            .encoding_details
            .as_ref()
            .and_then(|d| d.bit_depth);

        let frame_output = {
            let video = video.read();

            if let Some([w, h]) = video.video_renderer.dimensions() {
                video_resolution = glam::vec2(w as _, h as _);
            }

            let storage_engine = ctx.viewer_ctx.store_context.recording.storage_engine();
            // Both real fetches and "mark in use" calls go through here: the
            // `use_chunk_or_report_missing` call marks the chunk in use either
            // way; if `sub_id` is `None` we stop there.
            let lookup = |id: re_log_types::external::re_tuid::Tuid,
                          sub_id: Option<re_log_types::external::re_tuid::Tuid>|
             -> Option<&[u8]> {
                let Some(chunk) = storage_engine
                    .store()
                    .use_chunk_or_report_missing(&re_sdk_types::ChunkId::from_tuid(id))
                else {
                    output.set_missing_chunks(); // view-wide loading indicator
                    return None;
                };
                let sub_id = sub_id?;
                let (offsets, buffer) = re_arrow_util::blob_arrays_offsets_and_buffer(
                    chunk.raw_component_array(ctx.sample_component)?,
                )?;
                let row_idx = chunk.row_index_of(re_sdk_types::RowId::from_tuid(sub_id))?;
                let start = offsets[row_idx] as usize;
                let end = offsets[row_idx + 1] as usize;
                Some(&buffer.as_slice()[start..end])
            };

            video.video_renderer.frame_at(
                ctx.viewer_ctx.render_ctx(),
                video_stream_id(entity_path, ctx.sample_component, AT_TIME_CURSOR_SALT),
                video_stream_time_from_query(&query_context.query),
                &|source| match source {
                    re_video::VideoSource::Id { id, sub_id } => lookup(id, sub_id).unwrap_or(&[]),
                    // No `Span` sources for video streams.
                    re_video::VideoSource::Span(_) => &[],
                },
            )
        };

        let depth_offset = depth_offsets
            .per_entity_and_visualizer
            .get(&(ctx.visualizer_name, entity_path.hash()))
            .copied()
            .unwrap_or_default();
        let opacity = opacity.unwrap_or_else(|| Opacity(1.0.into()));
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        let multiplicative_tint = egui::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));

        let depth_config = ctx.depth_handler.as_ref().map(|depth_handler| {
            (depth_handler.get_depth_config)(
                ctx.view_context,
                &latest_at,
                data_result,
                instruction,
                &output,
            )
        });

        // In 3D views, depth images should render as point clouds when a pinhole camera is available.
        let rendered_as_depth_cloud = if view_kind == SpatialViewKind::ThreeD
            && let Some(depth_config) = &depth_config
            && let Some(frame_texture) = frame_output
                .output
                .as_ref()
                .and_then(|t| t.texture.as_ref())
        {
            let tree_root_frame = transform_info.tree_root();
            if let Some(pinhole_tree_root_info) = transforms.pinhole_tree_root_info(tree_root_frame)
                && let Some(world_from_view) = transforms.target_from_pinhole_root(tree_root_frame)
            {
                let colormapped = depth_config.to_colormapped_texture(frame_texture.clone());

                let pinhole = &pinhole_tree_root_info.pinhole_projection;
                let world_from_view = world_from_view.as_affine3a();
                let world_from_rdf = world_from_view
                    * glam::Affine3A::from_mat3(pinhole.view_coordinates.from_rdf());

                let dimensions = glam::UVec2::from_array(colormapped.texture.width_height());

                if dimensions.x == 0 || dimensions.y == 0 {
                    false
                } else {
                    let world_depth_from_texture_depth = 1.0 / *depth_config.depth_meter.0;

                    let fov_y = pinhole
                        .image_from_camera
                        .fov_y(pinhole.resolution.unwrap_or_else(|| [1.0, 1.0].into()));
                    let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * dimensions.y as f32);
                    let point_radius_from_world_depth =
                        *depth_config.fill_ratio.0 * pixel_width_from_depth;

                    let cloud = re_renderer::renderer::DepthCloud {
                        world_from_rdf,
                        depth_camera_intrinsics: pinhole.image_from_camera.0.into(),
                        world_depth_from_texture_depth,
                        point_radius_from_world_depth,
                        min_max_depth_in_world: [
                            world_depth_from_texture_depth * colormapped.range[0],
                            world_depth_from_texture_depth * colormapped.range[1],
                        ],
                        depth_dimensions: dimensions,
                        depth_texture: colormapped.texture.clone(),
                        colormap: depth_config.colormap,
                        outline_mask_id: highlight.overall,
                        picking_object_id: re_renderer::PickingLayerObjectId(entity_path.hash64()),
                    };

                    ctx.data.add_bounding_box(
                        entity_path.hash(),
                        cloud.world_space_bbox(),
                        glam::Affine3A::IDENTITY,
                    );
                    if let Some(depth_handler) = &mut ctx.depth_handler {
                        depth_handler.depth_cloud_entities.insert(
                            entity_path.hash(),
                            super::depth_images::DepthImageProcessResult {
                                image_info: None,
                                depth_meter: depth_config.depth_meter,
                                colormap: colormapped,
                            },
                        );
                    }
                    depth_clouds.push(cloud);
                    true
                }
            } else {
                false
            }
        } else {
            false
        };

        if !rendered_as_depth_cloud {
            show_video_frame(
                ctx.view_context,
                ctx.data,
                entity_path,
                world_from_entity,
                highlight,
                video_resolution,
                instruction.id,
                frame_output.output.map(|texture| VideoFrameRenderInfo {
                    texture,
                    depth_offset,
                    multiplicative_tint,
                }),
                frame_output.error.map(VideoPlaybackIssue::from),
                depth_config.as_ref(),
                bit_depth,
            );

            if ctx.context_systems.view_class_identifier == SpatialView2D::identifier() {
                let bounding_box = macaw::BoundingBox::from_min_size(
                    world_from_entity.transform_point3(glam::Vec3::ZERO),
                    video_resolution.extend(0.0),
                );
                ctx.data
                    .add_bounding_box(entity_path.hash(), bounding_box, world_from_entity);
            }
        }
    }

    if depth_clouds.is_empty() {
        Ok(output
            .with_draw_data([PickableTexturedRect::to_draw_data(
                viewer_ctx.render_ctx(),
                &ctx.data.pickable_rects,
            )?])
            .with_visualizer_data(std::mem::take(ctx.data)))
    } else {
        super::depth_images::populate_depth_visualizer_execution_result(
            &ctx,
            ctx.data,
            depth_clouds,
            output,
        )
        .map(|output| output.with_visualizer_data(std::mem::take(ctx.data)))
    }
}

pub const AT_TIME_CURSOR_SALT: u64 = 0x12356;

/// Identify a video stream for a given video.
///
/// `time_track_salt` refers to a unique identifier for a certain way to play through time.
///
/// For things following the given entity & component at the play head, use [`AT_TIME_CURSOR_SALT`]
fn video_stream_id(
    entity_path: &EntityPath,
    sample_component: re_sdk_types::ComponentIdentifier,
    time_track_salt: u64,
) -> re_video::player::VideoPlayerStreamId {
    re_video::player::VideoPlayerStreamId(
        re_log_types::hash::Hash64::hash((entity_path.hash(), sample_component, time_track_salt))
            .hash64(),
    )
}

/// Frame texture with rendering parameters for [`show_video_frame`].
struct VideoFrameRenderInfo {
    texture: re_renderer::video::VideoFrameTexture,
    depth_offset: re_renderer::DepthOffset,
    multiplicative_tint: egui::Rgba,
}

/// A video playback issue to display in [`show_video_frame`].
struct VideoPlaybackIssue {
    message: String,
    severity: VideoPlaybackIssueSeverity,
    should_request_more_frames: bool,

    /// Should we show the last successful video frame behind this issue?
    show_frame: bool,
}

impl VideoPlaybackIssue {
    pub fn custom(message: String, severity: VideoPlaybackIssueSeverity) -> Self {
        Self {
            message,
            severity,
            should_request_more_frames: false,
            show_frame: false,
        }
    }
}

impl From<VideoPlayerError> for VideoPlaybackIssue {
    fn from(error: VideoPlayerError) -> Self {
        Self {
            message: error.to_string(),
            severity: error.severity(),
            should_request_more_frames: error.should_request_more_frames(),
            show_frame: match error {
                VideoPlayerError::NegativeTimestamp
                | VideoPlayerError::InsufficientSampleData(_) => false,

                VideoPlayerError::EmptyBuffer
                | VideoPlayerError::UnloadedSampleData(_)
                | VideoPlayerError::CreateChunk(_)
                | VideoPlayerError::DecodeChunk(_)
                | VideoPlayerError::Decoding(_)
                | VideoPlayerError::BadData
                | VideoPlayerError::TextureUploadError(_)
                | VideoPlayerError::DecoderUnexpectedlyExited => true,
            },
        }
    }
}

/// Show a video frame and/or a playback issue.
///
/// - Both `None`: registers bounds only.
/// - Only `frame`: renders the frame texture.
/// - Only `issue`: shows the error/loading overlay.
/// - Both `Some`: renders the frame with the error overlaid on top.
#[expect(clippy::too_many_arguments)]
fn show_video_frame(
    ctx: &ViewContext<'_>,
    visualizer_data: &mut SpatialViewVisualizerData,
    entity_path: &EntityPath,
    world_from_entity: glam::Affine3A,
    highlight: &re_viewer_context::ViewOutlineMasks,
    fallback_video_size: glam::Vec2,
    visualizer_instruction: VisualizerInstructionId,
    frame: Option<VideoFrameRenderInfo>,
    issue: Option<VideoPlaybackIssue>,
    depth_config: Option<&DepthTextureConfig>,
    bit_depth: Option<u8>,
) {
    let show_frame = issue.as_ref().map(|issue| issue.show_frame).unwrap_or(true);
    if !show_frame {
        return;
    }

    // Use the texture dimensions if available, otherwise the provided fallback.
    let video_size = frame
        .as_ref()
        .and_then(|f| f.texture.texture.as_ref())
        .map(|t| glam::vec2(t.width() as _, t.height() as _))
        .unwrap_or(fallback_video_size);

    // Make sure to use the video instead of texture size here,
    // since the texture may be a placeholder which doesn't have the full size yet.
    let top_left_corner_position = world_from_entity.transform_point3(glam::Vec3::ZERO);
    let extent_u = world_from_entity.transform_vector3(glam::Vec3::X * video_size.x);
    let extent_v = world_from_entity.transform_vector3(glam::Vec3::Y * video_size.y);

    let mut has_rendered_texture = false;
    let mut depth_offset = 0;

    let loading_indicator_reason = if let Some(issue) = &issue {
        if matches!(issue.severity, VideoPlaybackIssueSeverity::Loading) {
            Some(issue.message.clone())
        } else {
            None
        }
    } else if let Some(frame) = &frame
        && frame.texture.show_loading_indicator
    {
        Some(format!("Decoder: {:?}", frame.texture.decoder_delay_state))
    } else {
        None
    };

    if let Some(reason) = loading_indicator_reason {
        visualizer_data.loading_indicators.push(LoadingIndicator {
            center: top_left_corner_position + 0.5 * (extent_u + extent_v),
            half_extent_u: 0.5 * extent_u,
            half_extent_v: 0.5 * extent_v,
            reason,
        });
    }

    if let Some(frame) = frame {
        let re_renderer::video::VideoFrameTexture {
            texture,
            decoder_delay_state,
            show_loading_indicator,
            frame_info: _,
            source_pixel_format: _,
        } = frame.texture;

        if decoder_delay_state.should_request_more_frames() {
            ctx.egui_ctx().request_repaint();
        }

        if let Some(texture) = texture {
            has_rendered_texture = true;
            let animated_valid_frame = ctx.egui_ctx().animate_bool(
                egui::Id::new(format!("{entity_path} video loading indicator"))
                    .with(visualizer_instruction),
                issue.is_none() && !show_loading_indicator,
            );
            depth_offset = frame.depth_offset;
            let textured_rect = renderer::TexturedRect {
                top_left_corner_position,
                extent_u,
                extent_v,
                colormapped_texture: if let Some(config) = depth_config {
                    config.to_colormapped_texture(texture)
                } else {
                    renderer::ColormappedTexture::from_video_frame(texture, bit_depth)
                },
                options: renderer::RectangleOptions {
                    texture_filter_magnification: renderer::TextureFilterMag::Nearest,
                    texture_filter_minification: renderer::TextureFilterMin::Linear,
                    outline_mask: highlight.overall,
                    depth_offset: frame.depth_offset,
                    multiplicative_tint: frame
                        .multiplicative_tint
                        // Fade out if we don't have an up to date frame without issues.
                        .multiply(0.5 + 0.5 * animated_valid_frame),
                },
            };
            visualizer_data.add_pickable_rect(
                PickableTexturedRect {
                    ent_path: entity_path.clone(),
                    textured_rect,
                    source_data: PickableRectSourceData::Video {
                        depth_meter: depth_config.map(|c| c.depth_meter),
                    },
                },
                ctx.view_class_identifier,
            );
        }
    }

    // Register bounds explicitly when no texture was rendered. This ensures
    // loading indicators and issues are positioned correctly.
    if !has_rendered_texture {
        register_video_bounds_with_bounding_box(
            entity_path.hash(),
            visualizer_data,
            world_from_entity,
            video_size,
            ctx.view_class_identifier,
        );
    }

    let Some(issue) = issue else {
        return;
    };

    if issue.should_request_more_frames {
        ctx.egui_ctx().request_repaint();
    }

    let style = match issue.severity {
        VideoPlaybackIssueSeverity::Error => UiLabelStyle::Error,
        VideoPlaybackIssueSeverity::Informational => UiLabelStyle::Default,
        VideoPlaybackIssueSeverity::Loading => {
            // Already added loading indicator if needed.
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
        return;
    };

    let video_error_rect_size = {
        // Scale the icon and its label together so the whole error fits inside the
        // video rect. The stale texture underneath keeps showing through on purpose.
        // The label sits beside the icon and spans 3x the icon width total, so width
        // has to account for that.
        let icon_size = glam::vec2(
            video_error_texture.width() as f32,
            video_error_texture.height() as f32,
        );
        let scale = (video_size.x / (icon_size.x * 3.0)).min(video_size.y / icon_size.y);
        icon_size * scale
    };

    // Center the icon in the middle of the video rectangle.
    // Don't ignore translation - if the user moved the video frame, we move the error message along.
    // But do ignore any rotation/scale on this, gets complicated to center and weird generally.
    let center = glam::Vec3::from(world_from_entity.translation).truncate() + video_size * 0.5;
    let error_icon_top_left = center - video_error_rect_size * 0.5;

    // Add a label that annotates a rectangle that is a bit bigger than the error icon.
    // This makes the label track the icon better than putting it at a point.
    let label_target_rect = egui::Rect::from_min_size(
        egui::pos2(
            error_icon_top_left.x - video_error_rect_size.x,
            error_icon_top_left.y,
        ),
        egui::vec2(video_error_rect_size.x * 3.0, video_error_rect_size.y),
    );

    visualizer_data.ui_labels.push(UiLabel {
        text: issue.message,
        style,
        target: UiLabelTarget::Rect(label_target_rect),
        labeled_instance: re_entity_db::InstancePathHash::entity_all(entity_path),
        visualizer_instruction,
    });

    let error_rect = renderer::TexturedRect {
        top_left_corner_position: error_icon_top_left.extend(0.0),
        extent_u: glam::Vec3::X * video_error_rect_size.x,
        extent_v: glam::Vec3::Y * video_error_rect_size.y,
        colormapped_texture: renderer::ColormappedTexture::from_unorm_rgba(video_error_texture),
        options: renderer::RectangleOptions {
            texture_filter_magnification: renderer::TextureFilterMag::Linear,
            texture_filter_minification: renderer::TextureFilterMin::Linear,
            outline_mask: highlight.overall,
            multiplicative_tint: egui::Rgba::from(ctx.egui_ctx().tokens().text_default).to_opaque(),
            depth_offset,
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
