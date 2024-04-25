//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

mod contexts;
mod eye;
mod heuristics;
mod instance_hash_conversions;
mod max_image_dimension_subscriber;
mod mesh_cache;
mod mesh_loader;
mod picking;
mod scene_bounding_boxes;
mod space_camera_3d;
mod space_view_2d;
mod space_view_3d;
mod spatial_topology;
mod ui;
mod ui_2d;
mod ui_3d;
mod visualizers;

use re_types::components::{Resolution, TensorData};
pub use space_view_2d::SpatialSpaceView2D;
pub use space_view_3d::SpatialSpaceView3D;

#[doc(hidden)] // Public for benchmarks
pub use visualizers::{LoadedPoints, Points3DComponentData};

// ---

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}

fn resolution_from_tensor(
    entity_db: &re_entity_db::EntityDb,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<Resolution> {
    // TODO(#5607): what should happen if the promise is still pending?
    entity_db
        .latest_at_component::<TensorData>(entity_path, query)
        .and_then(|tensor| {
            tensor
                .image_height_width_channels()
                .map(|hwc| Resolution([hwc[1] as f32, hwc[0] as f32].into()))
        })
}

/// Utility for querying a pinhole archetype instance.
///
/// TODO(andreas): It should be possible to convert [`re_query::ArchetypeView`] to its corresponding Archetype for situations like this.
/// TODO(andreas): This is duplicated into `re_viewport`
fn query_pinhole(
    entity_db: &re_entity_db::EntityDb,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    // TODO(#5607): what should happen if the promise is still pending?
    entity_db
        .latest_at_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(|image_from_camera| re_types::archetypes::Pinhole {
            image_from_camera: image_from_camera.value,
            resolution: entity_db
                .latest_at_component(entity_path, query)
                .map(|c| c.value)
                .or_else(|| resolution_from_tensor(entity_db, query, entity_path)),
            camera_xyz: entity_db
                .latest_at_component(entity_path, query)
                .map(|c| c.value),
        })
}

pub(crate) fn configure_background(
    ctx: &re_viewer_context::ViewerContext<'_>,
    query: &re_viewer_context::ViewQuery<'_>,
    default_background: re_types::blueprint::archetypes::Background,
) -> (Option<re_renderer::QueueableDrawData>, re_renderer::Rgba) {
    use re_renderer::renderer;
    use re_types::blueprint::{archetypes::Background, components::BackgroundKind};

    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let background = re_space_view::query_space_view_sub_archetype::<Background>(
        query.space_view_id,
        blueprint_db,
        blueprint_query,
    )
    .0;
    let background = background.ok().flatten().unwrap_or(default_background);

    let Background {
        kind,
        color: solid_color,
    } = background;

    match kind {
        BackgroundKind::GradientDark => (
            Some(
                renderer::GenericSkyboxDrawData::new(
                    ctx.render_ctx,
                    renderer::GenericSkyboxType::GradientDark,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        ),

        BackgroundKind::GradientBright => (
            Some(
                renderer::GenericSkyboxDrawData::new(
                    ctx.render_ctx,
                    renderer::GenericSkyboxType::GradientBright,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        ),

        BackgroundKind::SolidColor => (
            None,
            solid_color
                .or(default_background.color)
                .unwrap_or(re_types::components::Color::BLACK)
                .into(),
        ),
    }
}
