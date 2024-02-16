use itertools::Itertools;
use nohash_hasher::IntSet;
use re_entity_db::EntityProperties;
use re_log_types::{EntityPath, EntityPathFilter};
use re_types::{components::ViewCoordinates, Loggable};
use re_viewer_context::{
    PerSystemEntities, RecommendedSpaceView, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
    VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{
        default_visualized_entities_for_visualizer_kind, update_object_property_heuristics,
    },
    spatial_topology::{HeuristicHints, SpatialTopology, SubSpaceConnectionFlags},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::register_3d_spatial_visualizers,
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
            if !primary_space.supports_3d_content() {
                // If this is strict 2D space, only display the origin entity itself.
                // Everything else we have to assume requires some form of transformation.
                return VisualizableFilterContext3D {
                    entities_in_main_3d_space: std::iter::once(space_origin.clone()).collect(),
                    entities_under_pinholes: Default::default(),
                };
            }

            // All entities in the 3d space are visualizable + everything under pinholes.
            let mut entities_in_main_3d_space = primary_space.entities.clone();
            let mut entities_under_pinholes = IntSet::<EntityPath>::default();

            for child_origin in &primary_space.child_spaces {
                let Some(child_space) = topo.subspace_for_subspace_origin(child_origin.hash())
                else {
                    // Should never happen, implies that a child space is not in the list of subspaces.
                    continue;
                };

                if child_space
                    .connection_to_parent
                    .contains(SubSpaceConnectionFlags::Pinhole)
                {
                    // Note that for this the connection to the parent is allowed to contain the disconnected flag.
                    // Entities _at_ pinholes are a special case: we display both 3d and 2d visualizers for them.
                    entities_in_main_3d_space.insert(child_space.origin.clone());
                    entities_under_pinholes.extend(child_space.entities.iter().cloned());
                }
            }

            VisualizableFilterContext3D {
                entities_in_main_3d_space,
                entities_under_pinholes,
            }
        });

        Box::new(context.unwrap_or_default())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();

        let mut indicated_entities = default_visualized_entities_for_visualizer_kind(
            ctx,
            self.identifier(),
            SpatialSpaceViewKind::ThreeD,
        );

        // ViewCoordinates is a strong indicator that a 3D space view is needed.
        // Note that if the root has `ViewCoordinates`, this will stop the root splitting heuristic
        // from splitting the root space into several subspaces.
        //
        // TODO(andreas)/TODO(#4926):
        // It's tempting to add a visualizer for view coordinates so that it's already picked up via `entities_with_indicator_for_visualizer_kind`.
        // Is there a nicer way for this or do we want a visualizer for view coordinates anyways?
        // There's also a strong argument to be made that ViewCoordinates implies a 3D space, thus changing the SpacialTopology accordingly!
        ctx.entity_db
            .tree()
            .visit_children_recursively(&mut |path, info| {
                if info.components.contains_key(&ViewCoordinates::name()) {
                    indicated_entities.insert(path.clone());
                }
            });

        // Spawn a space view at each subspace that has any potential 3D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(ctx.entity_db.store_id(), |topo| SpaceViewSpawnHeuristics {
            recommended_space_views: topo
                .iter_subspaces()
                .filter_map(|subspace| {
                    if !subspace.supports_3d_content() || subspace.entities.is_empty() {
                        None
                    } else {
                        // Creates space views at each view coordinates if there's any.
                        // (yes, we do so even if they're empty at the moment!)
                        let mut roots = subspace
                            .heuristic_hints
                            .iter()
                            .filter(|(_, hint)| hint.contains(HeuristicHints::ViewCoordinates3d))
                            .map(|(root, _)| root.clone())
                            .collect::<Vec<_>>();

                        // If there's no view coordinates or there are still some entities not covered,
                        // create a view at the subspace origin.
                        if !roots.iter().contains(&subspace.origin)
                            && indicated_entities
                                .intersection(&subspace.entities)
                                .any(|e| roots.iter().all(|root| !e.starts_with(root)))
                        {
                            roots.push(subspace.origin.clone());
                        }

                        Some(roots.into_iter().map(|root| RecommendedSpaceView {
                            query_filter: EntityPathFilter::subtree_entity_filter(&root),
                            root,
                        }))
                    }
                })
                .flatten()
                .collect(),
        })
        .unwrap_or_default()
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
