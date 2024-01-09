use re_entity_db::{EntityProperties, EntityTree};
use re_log_types::EntityPath;
use re_types::{components::PinholeProjection, Loggable as _};
use re_viewer_context::{
    AutoSpawnHeuristic, PerSystemEntities, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
    VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, update_object_property_heuristics},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::{register_2d_spatial_visualizers, SpatialViewVisualizerData},
};

// TODO(#4388): 2D/3D relationships should be solved via a "SpatialTopology" construct.
pub struct VisualizableFilterContext2D {
    /// True if there's a pinhole camera at the origin, meaning we can display 3d content.
    pub has_pinhole_at_origin: bool,

    /// All subtrees that are invalid since they're behind a pinhole that's not at the origin.
    pub invalid_subtrees: Vec<EntityPath>,
}

impl VisualizableFilterContext for VisualizableFilterContext2D {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct SpatialSpaceView2D;

impl SpaceViewClass for SpatialSpaceView2D {
    type State = SpatialSpaceViewState;

    const IDENTIFIER: &'static str = "2D";
    const DISPLAY_NAME: &'static str = "2D";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_2D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_2d::help_text(re_ui)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        register_spatial_contexts(system_registry)?;
        register_2d_spatial_visualizers(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::State) -> Option<f32> {
        let size = state.bounding_boxes.accumulated.size();
        Some(size.x / size.y)
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn visualizable_filter_context(
        &self,
        space_origin: &EntityPath,
        entity_db: &re_entity_db::EntityDb,
    ) -> Box<dyn VisualizableFilterContext> {
        re_tracing::profile_function!();

        // See also `SpatialSpaceView3D::visualizable_filter_context`

        let origin_tree = entity_db.tree().subtree(space_origin);

        let has_pinhole_at_origin = origin_tree.map_or(false, |tree| {
            tree.entity
                .components
                .contains_key(&PinholeProjection::name())
        });

        fn visit_children_recursively(tree: &EntityTree, invalid_subtrees: &mut Vec<EntityPath>) {
            if tree
                .entity
                .components
                .contains_key(&PinholeProjection::name())
            {
                invalid_subtrees.push(tree.path.clone());
            } else {
                for child in tree.children.values() {
                    visit_children_recursively(child, invalid_subtrees);
                }
            }
        }

        let mut invalid_subtrees = Vec::new();
        if let Some(origin_tree) = origin_tree {
            for child_tree in origin_tree.children.values() {
                visit_children_recursively(child_tree, &mut invalid_subtrees);
            }
        };

        Box::new(VisualizableFilterContext2D {
            has_pinhole_at_origin,
            invalid_subtrees,
        })
    }

    fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        state: &Self::State,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut re_entity_db::EntityPropertyMap,
    ) {
        update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            &state.bounding_boxes.accumulated,
            SpatialSpaceViewKind::TwoD,
        );
    }

    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        space_origin: &EntityPath,
        per_system_entities: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        let mut score = auto_spawn_heuristic(
            self.identifier(),
            ctx,
            per_system_entities,
            SpatialSpaceViewKind::TwoD,
        );

        // If this is the root space view, and it contains a part that would
        // prefer to be 3D, don't spawn the 2D view.
        //
        // Since pinhole projections provide a mapping between a 2D child space and a 3D
        // parent space, it means that for any 3D content to be projected into a 2D space,
        // there must exist a common 3D ancestor-space which has a *child* which is a 2D space
        // (and contains a pinhole.) But if this space origin is at the root itself, that common
        // ancestor would need to be a parent of the root, which doesn't exist. As such, it's
        // impossible that this space would be able to correctly account for the required
        // content. By not spawning a 2D space in this case we ensure the 3D version gets chosen
        // by the heuristics instead.
        if space_origin.is_root() {
            let parts = ctx
                .space_view_class_registry
                .new_visualizer_collection(self.identifier());

            for part in per_system_entities.keys() {
                if let Ok(part) = parts.get_by_identifier(*part) {
                    if let Some(part_data) = part
                        .data()
                        .and_then(|d| d.downcast_ref::<SpatialViewVisualizerData>())
                    {
                        if part_data.preferred_view_kind == Some(SpatialSpaceViewKind::ThreeD) {
                            return AutoSpawnHeuristic::NeverSpawn;
                        }
                    }
                }
            }
        }

        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score) = &mut score {
            // If a 2D view contains nothing inherently 2D in nature, don't
            // spawn it, though in all other cases default to 2D over 3D as a tie-breaker.
            if *score == 0.0 {
                return AutoSpawnHeuristic::NeverSpawn;
            } else {
                *score += 0.1;
            }
        }

        score
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
        state.selection_ui(ctx, ui, space_origin, SpatialSpaceViewKind::TwoD);
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

        state.bounding_boxes.update(&system_output.view_systems);
        state.scene_num_primitives = system_output
            .context_systems
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_2d::view_2d(ctx, ui, state, query, system_output)
    }
}
