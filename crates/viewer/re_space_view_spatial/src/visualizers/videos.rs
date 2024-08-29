use glam::Vec3;
use re_log_types::hash::Hash64;
use re_log_types::TimeType;
use re_renderer::renderer::ColormappedTexture;
use re_renderer::renderer::RectangleOptions;
use re_renderer::renderer::TextureFilterMag;
use re_renderer::renderer::TextureFilterMin;
use re_renderer::renderer::TexturedRect;
use re_renderer::RenderContext;
use re_space_view::TimeKey;
use re_types::archetypes::AssetVideo;
use re_types::components::Blob;
use re_types::components::MediaType;
use re_types::ArrowBuffer;
use re_types::ArrowString;
use re_types::Loggable as _;
use re_viewer_context::SpaceViewClass as _;
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    ViewContext, ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::video_cache::VideoCache;
use crate::video_cache::VideoCacheKey;
use crate::visualizers::entity_iterator::iter_buffer;
use crate::SpatialSpaceView2D;
use crate::{
    contexts::SpatialSceneEntityContext, view_kind::SpatialSpaceViewKind,
    visualizers::filter_visualizable_2d_entities,
};

use super::bounding_box_for_textured_rect;
use super::{entity_iterator::process_archetype, SpatialViewVisualizerData};

pub struct AssetVideoVisualizer {
    pub data: SpatialViewVisualizerData,
}

struct AssetVideoComponentData {
    index: TimeKey,
    blob: ArrowBuffer<u8>,
    media_type: Option<ArrowString>,
}

impl Default for AssetVideoVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl IdentifiedViewSystem for AssetVideoVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Video".into()
    }
}

impl VisualizerSystem for AssetVideoVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<AssetVideo>()
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

        let mut rectangles = Vec::new();

        process_archetype::<Self, AssetVideo, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;
                let Some(all_blob_chunks) = results.get_required_chunks(&Blob::name()) else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_blobs_indexed = iter_buffer::<u8>(&all_blob_chunks, timeline, Blob::name());
                let all_media_types = results.iter_as(timeline, MediaType::name());

                let data = re_query::range_zip_1x1(all_blobs_indexed, all_media_types.string())
                    .filter_map(|(index, blobs, media_types)| {
                        blobs.first().map(|blob| AssetVideoComponentData {
                            index,
                            blob: blob.clone(),
                            media_type: media_types
                                .and_then(|media_types| media_types.first().cloned()),
                        })
                    });

                let current_time_nanoseconds = match timeline.typ() {
                    TimeType::Time => view_query.latest_at.as_f64(),
                    // TODO(jan): scale by ticks per second
                    #[allow(clippy::match_same_arms)]
                    TimeType::Sequence => view_query.latest_at.as_f64(),
                };
                let current_time_seconds = current_time_nanoseconds / 1e9;

                self.process_data(
                    ctx,
                    render_ctx,
                    &mut rectangles,
                    spatial_ctx,
                    data,
                    current_time_seconds,
                    results.query_result_hash(),
                );

                Ok(())
            },
        )?;

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!("Failed to create rectangle draw data from images: {err}");
            }
        }

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl AssetVideoVisualizer {
    #[allow(clippy::unused_self)]
    #[allow(clippy::too_many_arguments)]
    fn process_data(
        &mut self,
        ctx: &QueryContext<'_>,
        render_ctx: &RenderContext,
        rectangles: &mut Vec<TexturedRect>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = AssetVideoComponentData>,
        current_time_seconds: f64,
        query_result_hash: Hash64,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let timestamp_s = current_time_seconds - data.index.time.as_f64() / 1e9;
            let video = AssetVideo {
                blob: data.blob.clone().into(),
                media_type: data.media_type.clone().map(Into::into),
            };

            let primary_row_id = data.index.row_id;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);

            let video = ctx.viewer_ctx.cache.entry(|c: &mut VideoCache| {
                c.entry(
                    &entity_path.to_string(),
                    VideoCacheKey {
                        versioned_instance_path_hash: picking_instance_hash
                            .versioned(primary_row_id),
                        query_result_hash,
                        media_type: data.media_type.clone().map(Into::into),
                    },
                    &video.blob,
                    video.media_type.as_ref().map(|v| v.as_str()),
                    render_ctx,
                )
            });

            if let Some(video) = video {
                let mut video = video.lock();
                let texture = video.frame_at(timestamp_s);

                let world_from_entity = ent_context
                    .transform_info
                    .single_entity_transform_required(ctx.target_entity_path, "Video");
                let textured_rect = TexturedRect {
                    top_left_corner_position: world_from_entity.transform_point3(Vec3::ZERO),
                    extent_u: world_from_entity.transform_vector3(Vec3::X * video.width() as f32),
                    extent_v: world_from_entity.transform_vector3(Vec3::Y * video.height() as f32),

                    colormapped_texture: ColormappedTexture::from_unorm_rgba(texture),
                    options: RectangleOptions {
                        texture_filter_magnification: TextureFilterMag::Nearest,
                        texture_filter_minification: TextureFilterMin::Linear,
                        ..Default::default()
                    },
                };

                if ent_context.space_view_class_identifier == SpatialSpaceView2D::identifier() {
                    self.data.add_bounding_box(
                        entity_path.hash(),
                        bounding_box_for_textured_rect(&textured_rect),
                        world_from_entity,
                    );
                }

                rectangles.push(textured_rect);
            };
        }
    }
}

re_viewer_context::impl_component_fallback_provider!(AssetVideoVisualizer => []);
