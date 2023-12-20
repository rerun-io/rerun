use nohash_hasher::IntSet;
use re_arrow_store::{DataStore, LatestAtQuery};
use re_data_store::{EntityProperties, EntityTree};
use re_log_types::{EntityPath, EntityPathHash, Timeline};
use re_types::{components::PinholeProjection, Loggable as _};
use re_viewer_context::{
    AutoSpawnHeuristic, IdentifiedViewSystem as _, PerSystemEntities, SpaceViewClass,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, update_object_property_heuristics},
    parts::{calculate_bounding_box, register_3d_spatial_parts, CamerasPart},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
};

// TODO(andreas): This context is used to determine whether a 2D entity has a valid transform
// and is thus visualizable. This should be expanded to cover any invalid transform as non-visualizable.
pub struct VisualizableFilterContext3D {
    /// Set of all entities that are under a pinhole camera.
    pub entities_under_pinhole: IntSet<EntityPathHash>,
}

#[derive(Default)]
pub struct SpatialSpaceView3D;

fn has_pinhole(tree: &EntityTree) -> bool {
    tree.entity
        .components
        .contains_key(&PinholeProjection::name())
}

impl SpaceViewClass for SpatialSpaceView3D {
    type State = SpatialSpaceViewState;

    const IDENTIFIER: &'static str = "3D";
    const DISPLAY_NAME: &'static str = "3D";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_3D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_3d::help_text(re_ui)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        register_spatial_contexts(system_registry)?;
        register_3d_spatial_parts(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn visualizable_filter_context(
        &self,
        space_origin: &EntityPath,
        store_db: &re_data_store::StoreDb,
    ) -> Box<dyn std::any::Any> {
        re_tracing::profile_function!();

        let mut entities_under_pinhole = IntSet::default();

        fn visit_children_recursively(
            tree: &EntityTree,
            entities_under_pinhole: &mut IntSet<EntityPathHash>,
        ) {
            if has_pinhole(tree) {
                // This and all children under it are under a pinhole camera!
                tree.visit_children_recursively(&mut |ent_path| {
                    entities_under_pinhole.insert(ent_path.hash());
                });
            } else {
                for child in tree.children.values() {
                    visit_children_recursively(child, entities_under_pinhole);
                }
            }
        }

        let entity_tree = &store_db.tree();

        // Find the entity path tree for the root.
        let Some(mut current_tree) = &entity_tree.subtree(space_origin) else {
            return Box::new(());
        };

        // Walk down the tree from the origin.
        visit_children_recursively(current_tree, &mut entities_under_pinhole);

        // Walk up from the reference to the highest reachable parent.
        // At each stop, add all child trees to the set.
        while let Some(parent_path) = current_tree.path.parent() {
            let Some(parent_tree) = entity_tree.subtree(&parent_path) else {
                return Box::new(());
            };

            if has_pinhole(parent_tree) {
                // What if we encounter a pinhole camera on the way up, i.e. an inverted pinhole?
                // At this point we can just stop, because there's no valid transform to these entities anyways!
                break;
            }

            for child in parent_tree.children.values() {
                if child.path == current_tree.path {
                    // Don't add the current tree again.
                    continue;
                }
                visit_children_recursively(child, &mut entities_under_pinhole);
            }

            current_tree = parent_tree;
        }

        Box::new(VisualizableFilterContext3D {
            entities_under_pinhole,
        })
    }

    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        space_origin: &EntityPath,
        per_system_entities: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        let score = auto_spawn_heuristic(
            self.identifier(),
            ctx,
            per_system_entities,
            SpatialSpaceViewKind::ThreeD,
        );

        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(mut score) = score {
            if let Some(camera_paths) = per_system_entities.get(&CamerasPart::identifier()) {
                // If there is a camera at the origin, this cannot be a 3D space -- it must be 2D
                if camera_paths.contains(space_origin) {
                    return AutoSpawnHeuristic::NeverSpawn;
                } else if !camera_paths.is_empty() {
                    // If there's a camera at a non-root path, make 3D view higher priority.
                    // TODO(andreas): It would be nice to just return `AutoSpawnHeuristic::AlwaysSpawn` here
                    // but AlwaysSpawn does not prevent other `SpawnClassWithHighestScoreForRoot` instances
                    // from being added to the view.
                    score += 100.0;
                }
            }

            AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score)
        } else {
            score
        }
    }

    fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        state: &Self::State,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            &state.scene_bbox_accum,
            SpatialSpaceViewKind::ThreeD,
        );
    }

    fn selection_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        state.selection_ui(ctx, ui, space_origin, SpatialSpaceViewKind::ThreeD);
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        state.scene_bbox =
            calculate_bounding_box(&system_output.view_systems, &mut state.scene_bbox_accum);
        state.scene_num_primitives = system_output
            .context_systems
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_3d::view_3d(ctx, ui, state, query, system_output)
    }
}
