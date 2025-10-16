use re_types::{
    Archetype as _,
    archetypes::VideoStream,
    components::{self, Opacity},
    image::ImageKind,
};
use re_view::{DataResultQuery as _, RangeResultsExt as _};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, TypedComponentFallbackProvider,
    VideoStreamCache, VideoStreamProcessingError, ViewClass as _, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem, video_stream_time_from_query,
};

use crate::{
    PickableTexturedRect, SpatialView2D, SpatialViewState,
    contexts::{EntityDepthOffsets, TransformTreeContext},
    view_kind::SpatialViewKind,
    visualizers::{
        SpatialViewVisualizerData, filter_visualizable_2d_entities,
        video::{
            VideoPlaybackIssueSeverity, show_video_playback_issue, video_stream_id,
            visualize_video_frame_texture,
        },
    },
};

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

            let world_from_entity = transform_info
                .single_transform_required_for_entity(entity_path, VideoStream::name());
            let query_context = ctx.query_context(data_result, &latest_at);
            let highlight = view_query
                .highlights
                .entity_outline_mask(entity_path.hash());

            // Note that we may or may not know the video size independently of error occurrence.
            // (if it's just a decoding error we may still know the size from the container!)
            // In case we haven error we want to center the message in the middle, so we need some area.
            // Note that this area is also used for the bounding box which is important for the 2D view to determine default bounds.
            let mut video_resolution = glam::vec2(1280.0, 720.0);

            let opacity_result = data_result.latest_at_with_blueprint_resolved_data_for_component(
                ctx,
                &latest_at,
                &VideoStream::descriptor_opacity(),
            );
            let all_opacities =
                opacity_result.iter_as(view_query.timeline, VideoStream::descriptor_opacity());
            let opacity = all_opacities
                .slice::<f32>()
                .next()
                .and_then(|((_time, _row_id), opacity)| opacity.first())
                .copied()
                .map(Opacity::from);

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
                    let (description, severity) = match err {
                        VideoStreamProcessingError::NoVideoSamplesFound => (
                            format!("No video samples available for {entity_path:?}"),
                            VideoPlaybackIssueSeverity::Informational,
                        ),
                        _ => (
                            format!("Failed to play video at {entity_path:?}: {err}"),
                            VideoPlaybackIssueSeverity::Error,
                        ),
                    };

                    show_video_playback_issue(
                        ctx,
                        &mut self.data,
                        highlight,
                        world_from_entity,
                        description,
                        severity,
                        video_resolution,
                        entity_path,
                    );
                    continue;
                }
            };

            let video_time = video_stream_time_from_query(query_context.query);
            if video_time.0 < 0 {
                // The frame is from before the video starts, so nothing to draw here!
                continue;
            }

            let frame_result = {
                let video = video.read();

                if let Some([w, h]) = video.video_renderer.dimensions() {
                    video_resolution = glam::vec2(w as _, h as _);
                }

                video.video_renderer.frame_at(
                    ctx.viewer_ctx.render_ctx(),
                    video_stream_id(entity_path, ctx.view_id, Self::identifier()),
                    video_stream_time_from_query(query_context.query),
                    &video.sample_buffers(),
                )
            };

            match frame_result {
                Ok(frame_texture) => {
                    let depth_offset = depth_offsets
                        .per_entity_and_visualizer
                        .get(&(Self::identifier(), entity_path.hash()))
                        .copied()
                        .unwrap_or_default();
                    let opacity = opacity.unwrap_or_else(|| {
                        self.fallback_for(&re_viewer_context::QueryContext {
                            view_ctx: ctx,
                            target_entity_path: entity_path,
                            archetype_name: Some(VideoStream::name()),
                            query: &latest_at,
                        })
                    });
                    #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
                    let multiplicative_tint =
                        egui::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));
                    visualize_video_frame_texture(
                        ctx,
                        &mut self.data,
                        frame_texture,
                        entity_path,
                        depth_offset,
                        world_from_entity,
                        highlight,
                        video_resolution,
                        multiplicative_tint,
                    );
                }

                Err(err) => {
                    show_video_playback_issue(
                        ctx,
                        &mut self.data,
                        highlight,
                        world_from_entity,
                        err.to_string(),
                        VideoPlaybackIssueSeverity::Error,
                        video_resolution,
                        entity_path,
                    );
                }
            }

            if context_systems.view_class_identifier == SpatialView2D::identifier() {
                let bounding_box = macaw::BoundingBox::from_min_size(
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

impl VideoStreamVisualizer {}

impl TypedComponentFallbackProvider<components::DrawOrder> for VideoStreamVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> components::DrawOrder {
        components::DrawOrder::DEFAULT_VIDEO
    }
}

impl TypedComponentFallbackProvider<components::Opacity> for VideoStreamVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> components::Opacity {
        // Streams should be transparent whenever they're on top of other media,
        // But fully opaque if there is no other media in the scene.
        let Some(view_state) = ctx.view_state().as_any().downcast_ref::<SpatialViewState>() else {
            return 1.0.into();
        };

        // Video streams are basically color images.
        //
        // Check [`crates/viewer/re_view_spatial/src/visualizers/images.rs`] for possible issues with this approach.
        view_state
            .fallback_opacity_for_image_kind(ImageKind::Color)
            .into()
    }
}

re_viewer_context::impl_component_fallback_provider!(VideoStreamVisualizer => [components::DrawOrder, Opacity]);
