use nohash_hasher::IntSet;
use re_entity_db::EntityProperties;
use re_log_types::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, IdentifiedViewSystem as _, PerSystemEntities, SpaceViewClass,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, spawn_heuristics, update_object_property_heuristics},
    spatial_topology::{SpatialTopology, SubSpaceDimensionality},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::{register_3d_spatial_visualizers, CamerasVisualizer},
};

#[derive(Default)]
pub struct VisualizableFilterContext3D {
    // TODO(andreas): Would be nice to use `EntityPathHash` in order to avoid bumping reference counters.
    pub entities_in_main_3d_space: IntSet<EntityPath>,
    pub entities_under_pinholes: IntSet<EntityPath>,
}

impl VisualizableFilterContext for VisualizableFilterContext3D {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct SpatialSpaceView3D;

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
        // Ensure spatial topology is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();

        register_spatial_contexts(system_registry)?;
        register_3d_spatial_visualizers(system_registry)?;

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
        entity_db: &re_entity_db::EntityDb,
    ) -> Box<dyn VisualizableFilterContext> {
        re_tracing::profile_function!();

        // TODO(andreas): The `VisualizableFilterContext` depends entirely on the spatial topology.
        // If the topology hasn't changed, we don't need to recompute any of this.
        // Also, we arrive at the same `VisualizableFilterContext` for lots of different origins!

        let context = SpatialTopology::access(entity_db.store_id(), |topo| {
            let primary_space = topo.subspace_for_entity(space_origin);
            match primary_space.dimensionality {
                SubSpaceDimensionality::Unknown => VisualizableFilterContext3D {
                    entities_in_main_3d_space: primary_space.entities.clone(),
                    entities_under_pinholes: Default::default(),
                },

                SubSpaceDimensionality::ThreeD => {
                    // All entities in the 3d space are visualizable + everything under pinholes.
                    let mut entities_in_main_3d_space = primary_space.entities.clone();
                    let mut entities_under_pinholes = IntSet::<EntityPath>::default();

                    for (child_origin, connection) in &primary_space.child_spaces {
                        if connection.is_connected_pinhole() {
                            let Some(child_space) =
                                topo.subspace_for_subspace_origin(child_origin.hash())
                            else {
                                // Should never happen, implies that a child space is not in the list of subspaces.
                                continue;
                            };

                            // Entities _at_ pinholes are a special case: we display both 3d and 2d visualizers for them.
                            entities_in_main_3d_space.insert(child_space.origin.clone());
                            entities_under_pinholes.extend(child_space.entities.iter().cloned());
                        }
                    }

                    VisualizableFilterContext3D {
                        entities_in_main_3d_space,
                        entities_under_pinholes,
                    }
                }

                SubSpaceDimensionality::TwoD => {
                    // If this is 2D space, only display the origin entity itself.
                    // Everything else we have to assume requires some form of transformation.
                    VisualizableFilterContext3D {
                        entities_in_main_3d_space: std::iter::once(space_origin.clone()).collect(),
                        entities_under_pinholes: Default::default(),
                    }
                }
            }
        });

        Box::new(context.unwrap_or_default())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        spawn_heuristics(ctx, self.identifier(), SpatialSpaceViewKind::TwoD)
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
            if let Some(camera_paths) = per_system_entities.get(&CamerasVisualizer::identifier()) {
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
        entity_properties: &mut re_entity_db::EntityPropertyMap,
    ) {
        update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            &state.bounding_boxes.accumulated,
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

        state.bounding_boxes.update(&system_output.view_systems);
        state.scene_num_primitives = system_output
            .context_systems
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_3d::view_3d(ctx, ui, state, query, system_output)
    }
}
