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
    PickableTexturedRect, SpatialView2D,
    contexts::{EntityDepthOffsets, TransformTreeContext},
    view_kind::SpatialViewKind,
    visualizers::{
        SpatialViewVisualizerData, filter_visualizable_2d_entities,
        video::{show_video_error, video_stream_id, visualize_video_frame_texture},
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
                    show_video_error(
                        ctx,
                        &mut self.data,
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
            // Video streams are always using the timeline directly for their timestamps,
            // therefore, we can use the unaltered time for all timeline types.
            let video_time = re_video::Time::new(query_context.query.at().as_i64());

            let frame_result = {
                let video = video.read();

                if let Some([w, h]) = video.video_renderer.dimensions() {
                    video_resolution = glam::vec2(w as _, h as _);
                }

                video.video_renderer.frame_at(
                    ctx.viewer_ctx.render_ctx(),
                    video_stream_id(entity_path, ctx.view_id, Self::identifier()),
                    video_time,
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
                        ctx,
                        &mut self.data,
                        video_frame_reference,
                        entity_path,
                        depth_offset,
                        world_from_entity,
                        highlight,
                        video_resolution,
                    );
                }

                Err(err) => {
                    show_video_error(
                        ctx,
                        &mut self.data,
                        highlight,
                        world_from_entity,
                        err.to_string(),
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

re_viewer_context::impl_component_fallback_provider!(VideoStreamVisualizer => [components::DrawOrder]);
