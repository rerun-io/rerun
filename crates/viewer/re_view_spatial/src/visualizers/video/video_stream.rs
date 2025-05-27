use re_log_types::{EntityPath, hash::Hash64};
use re_renderer::{
    renderer::{
        ColormappedTexture, RectangleOptions, TextureFilterMag, TextureFilterMin, TexturedRect,
    },
    resource_managers::ImageDataDesc,
};
use re_types::{
    Archetype as _,
    archetypes::VideoStream,
    components::{self},
};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, TypedComponentFallbackProvider,
    VideoStreamCache, ViewClass as _, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    PickableRectSourceData, PickableTexturedRect, SpatialView2D,
    contexts::{EntityDepthOffsets, TransformTreeContext},
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::{
        SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget,
        filter_visualizable_2d_entities,
        video::{video_stream_id, visualize_video_frame_texture},
    },
};

// TODO(#9832): Support opacity for videos
// TODO(jan): Fallback opacity in the same way as color/depth/segmentation images
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
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<VideoStream>()
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
        re_tracing::profile_function!();

        let viewer_ctx = ctx.viewer_ctx;
        let transforms = context_systems.get::<TransformTreeContext>()?;
        let depth_offsets = context_systems.get::<EntityDepthOffsets>()?;
        let latest_at = view_query.latest_at_query();

        for data_result in view_query.iter_visible_data_results(Self::identifier()) {
            let entity_path = &data_result.entity_path;

            let Some(transform_info) = transforms.transform_info_for_entity(entity_path.hash())
            else {
                continue;
            };

            let world_from_entity =
                transform_info.single_entity_transform_required(entity_path, VideoStream::name());
            let query_context = ctx.query_context(data_result, &latest_at);
            let highlight = view_query
                .highlights
                .entity_outline_mask(entity_path.hash());

            // Note that we may or may not know the video size independently of error occurrence.
            // (if it's just a decoding error we may still know the size from the container!)
            // In case we haven error we want to center the message in the middle, so we need some area.
            // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
            let mut video_resolution = glam::vec2(1280.0, 720.0);

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
                    self.show_video_error(
                        &query_context,
                        highlight,
                        world_from_entity,
                        format!("Failed to load video stream at {entity_path:?}: {err}"),
                        video_resolution,
                        entity_path,
                    );
                    continue;
                }
            };

            // Video streams are handled like "infinite" videos both forward and backwards in time.
            // Therefore, the "time in the video" is whatever time we have on the timeline right now.
            let time_since_video_start_in_secs = query_context.query.at().as_f64();

            // TODO: almost same code as on video frame reference
            let frame_result = {
                let video = video.read();

                if let Some([w, h]) = video.video_renderer.dimensions() {
                    video_resolution = glam::vec2(w as _, h as _);
                }

                video.video_renderer.frame_at(
                    ctx.viewer_ctx.render_ctx(),
                    video_stream_id(entity_path, ctx.view_id, Self::identifier()),
                    time_since_video_start_in_secs,
                    &video.sample_buffer_slices(),
                )
            };

            match frame_result {
                Ok(video_frame_reference) => {
                    let depth_offset = depth_offsets
                        .per_entity_and_visualizer
                        .get(&(Self::identifier(), entity_path.hash()))
                        .copied()
                        .unwrap_or_default();
                    visualize_video_frame_texture(
                        ctx.viewer_ctx,
                        &mut self.data,
                        video_frame_reference,
                        entity_path,
                        depth_offset,
                        world_from_entity,
                        highlight,
                        &mut video_resolution,
                    );
                }

                Err(err) => {
                    self.show_video_error(
                        &query_context,
                        highlight,
                        world_from_entity,
                        err.to_string(),
                        video_resolution,
                        entity_path,
                    );
                }
            }

            if context_systems.view_class_identifier == SpatialView2D::identifier() {
                let bounding_box = re_math::BoundingBox::from_min_size(
                    world_from_entity.transform_point3(glam::Vec3::ZERO),
                    video_resolution.extend(0.0),
                );
                self.data
                    .add_bounding_box(entity_path.hash(), bounding_box, world_from_entity);
            }
        }

        Ok(vec![PickableTexturedRect::to_draw_data(
            viewer_ctx.render_ctx(),
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

impl VideoStreamVisualizer {
    // TODO: almost fully duplicated from video_frame_reference!
    fn show_video_error(
        &mut self,
        ctx: &re_viewer_context::QueryContext<'_>,
        highlight: &re_viewer_context::ViewOutlineMasks,
        world_from_entity: glam::Affine3A,
        error_string: String,
        video_size: glam::Vec2,
        entity_path: &EntityPath,
    ) {
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
                outline_mask: highlight.overall,
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

impl TypedComponentFallbackProvider<components::DrawOrder> for VideoStreamVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> components::DrawOrder {
        components::DrawOrder::DEFAULT_VIDEO
    }
}

re_viewer_context::impl_component_fallback_provider!(VideoStreamVisualizer => [components::DrawOrder]);
