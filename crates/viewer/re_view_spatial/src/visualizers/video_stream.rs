use re_chunk_store::RangeQuery;
use re_log_types::{EntityPath, ResolvedTimeRange, hash::Hash64};
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
use re_video::VideoData;
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, TypedComponentFallbackProvider, VideoCache,
    ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    PickableRectSourceData, PickableTexturedRect,
    contexts::{EntityDepthOffsets, TransformTreeContext},
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::filter_visualizable_2d_entities,
};

use super::{SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};

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
                transform_info.single_entity_transform_required(&entity_path, VideoStream::name());
            let query_context = ctx.query_context(data_result, &latest_at);
            let highlight = view_query
                .highlights
                .entity_outline_mask(entity_path.hash());

            // Note that we may or may not know the video size independently of error occurrence.
            // (if it's just a decoding error we may still know the size from the container!)
            // In case we haven error we want to center the message in the middle, so we need some area.
            // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
            let mut video_resolution = glam::vec2(1280.0, 720.0);

            // Query for all video chunks on the **entire** timeline.
            // Tempting to bypass the query cache for this, but we don't expect to get new video chunks every frame
            // even for a running stream, so let's stick with the cache!
            //
            // TODO(andreas): Can we be more clever about the chunk range here?
            // Kinda tricky since we need to know how far back (and ahead for b-frames) we have to look.
            let entire_timeline_query =
                RangeQuery::new(view_query.timeline, ResolvedTimeRange::EVERYTHING);
            let all_video_chunks = ctx.recording().storage_engine().cache().range(
                &entire_timeline_query,
                entity_path,
                &[VideoStream::descriptor_chunk_data()],
            );
            let Ok(video_chunks) =
                all_video_chunks.get_required(&VideoStream::descriptor_chunk_data())
            else {
                self.show_video_error(
                    &query_context,
                    &highlight,
                    world_from_entity,
                    format!("No video chunks at {entity_path:?}"),
                    video_resolution,
                    entity_path,
                );

                // No video chunks found, skip this entity.
                continue;
            };

            // Setup video decoder...

            // TODO: this needs improvements.
            // TODO: should we try reading out the first few packages to guess some stuff?
            // let video_data = re_video::VideoData {
            //     config: re_video::Mp4Config {
            //         dimensions: Some([1280, 720]),
            //         ..Default::default()
            //     },
            // };

            // let video = ctx
            //     .viewer_ctx
            //     .store_context
            //     .caches
            //     .entry(|c: &mut VideoCache| {
            //         let debug_name = entity_path.to_string();
            //         c.entry(
            //             debug_name,
            //             blob_row_id,
            //             &VideoStream::descriptor_chunk_data(),
            //             &blob,
            //             media_type.as_ref(),
            //             ctx.app_options().video_decoder_settings(),
            //         )
            //     });

            // TODO:
            self.show_video_error(
                &query_context,
                highlight,
                world_from_entity,
                format!("Got {:?} chunks", video_chunks.len()),
                video_resolution,
                entity_path,
            );
        }

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
        let video_error_texture_result = render_ctx
            .texture_manager_2d
            .get_or_try_create_with::<image::ImageError>(
                Hash64::hash("video_error").hash64(),
                render_ctx,
                || {
                    let mut reader = image::ImageReader::new(std::io::Cursor::new(
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
                multiplicative_tint: egui::Rgba::from_rgb(0.5, 0.5, 0.5),
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
