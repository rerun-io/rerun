use nohash_hasher::IntSet;
use re_log_types::{EntityPath, Timeline};
use re_viewer_context::{AutoSpawnHeuristic, SpaceViewClassName, ViewerContext};

use crate::{parts::SpatialViewPartData, view_kind::SpatialSpaceViewKind};

pub fn auto_spawn_heuristic(
    class: &SpaceViewClassName,
    ctx: &ViewerContext<'_>,
    ent_paths: &IntSet<EntityPath>,
    view_kind: SpatialSpaceViewKind,
) -> AutoSpawnHeuristic {
    re_tracing::profile_function!();

    let store = ctx.store_db.store();
    let timeline = Timeline::log_time();

    let mut score = 0.0;

    let parts = ctx
        .space_view_class_registry
        .get_system_registry_or_log_error(class)
        .new_part_collection();
    let parts_2d = parts
        .iter()
        .filter(|part| {
            part.data()
                .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
                .map_or(false, |data| data.preferred_view_kind == Some(view_kind))
        })
        .collect::<Vec<_>>();

    for ent_path in ent_paths {
        let Some(components) = store.all_components(&timeline, ent_path) else {
                continue;
            };

        for part in &parts_2d {
            if part.queries_any_components_of(store, ent_path, &components) {
                score += 1.0;
                break;
            }
        }
    }

    AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score)
}
