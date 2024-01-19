use nohash_hasher::IntSet;
use re_entity_db::EntityProperties;
use re_log_types::EntityPath;
use re_viewer_context::{
    PerSystemEntities, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSystemExecutionError, ViewQuery, ViewerContext, VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{spawn_heuristics, update_object_property_heuristics},
    spatial_topology::{SpatialTopology, SubSpaceDimensionality},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::register_2d_spatial_visualizers,
};

#[derive(Default)]
pub struct VisualizableFilterContext2D {
    // TODO(andreas): Would be nice to use `EntityPathHash` in order to avoid bumping reference counters.
    pub entities_in_main_2d_space: IntSet<EntityPath>,
    pub reprojectable_3d_entities: IntSet<EntityPath>,
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
        // Ensure spatial topology is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();

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

        // TODO(andreas): The `VisualizableFilterContext` depends entirely on the spatial topology.
        // If the topology hasn't changed, we don't need to recompute any of this.
        // Also, we arrive at the same `VisualizableFilterContext` for lots of different origins!

        let context = SpatialTopology::access(entity_db.store_id(), |topo| {
            let primary_space = topo.subspace_for_entity(space_origin);
            match primary_space.dimensionality {
                SubSpaceDimensionality::Unknown => VisualizableFilterContext2D {
                    entities_in_main_2d_space: primary_space.entities.clone(),
                    reprojectable_3d_entities: Default::default(),
                },

                SubSpaceDimensionality::TwoD => {
                    // All entities in the 2d space are visualizable + the parent space if it is connected via a pinhole.
                    // For the moment we don't allow going down pinholes again.
                    let reprojected_3d_entities = primary_space
                        .parent_space
                        .and_then(|parent_space_origin| {
                            let is_connected_pinhole = topo
                                .subspace_for_subspace_origin(parent_space_origin)
                                .and_then(|parent_space| {
                                    parent_space
                                        .child_spaces
                                        .get(&primary_space.origin)
                                        .map(|connection| connection.is_connected_pinhole())
                                })
                                .unwrap_or(false);

                            if is_connected_pinhole {
                                topo.subspace_for_subspace_origin(parent_space_origin)
                                    .map(|parent_space| parent_space.entities.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    VisualizableFilterContext2D {
                        entities_in_main_2d_space: primary_space.entities.clone(),
                        reprojectable_3d_entities: reprojected_3d_entities,
                    }
                }

                SubSpaceDimensionality::ThreeD => {
                    // If this is 3D space, only display the origin entity itself.
                    // Everything else we have to assume requires some form of transformation.
                    VisualizableFilterContext2D {
                        entities_in_main_2d_space: std::iter::once(space_origin.clone()).collect(),
                        reprojectable_3d_entities: Default::default(),
                    }
                }
            }
        });

        Box::new(context.unwrap_or_default())
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

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        spawn_heuristics(ctx, self.identifier(), SpatialSpaceViewKind::TwoD)
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
