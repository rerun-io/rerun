use itertools::Itertools;
use nohash_hasher::IntSet;
use re_entity_db::{EntityDb, EntityProperties};
use re_log_types::EntityPath;
use re_space_view::query_space_view_sub_archetype;
use re_types::{
    blueprint::{archetypes::Background3D, components::Background3DKind},
    components::ViewCoordinates,
    Loggable,
};
use re_viewer_context::{
    PerSystemEntities, RecommendedSpaceView, SpaceViewClass, SpaceViewClassIdentifier,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState,
    SpaceViewStateExt as _, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
    VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{
        default_visualized_entities_for_visualizer_kind, update_object_property_heuristics,
    },
    spatial_topology::{HeuristicHints, SpatialTopology, SubSpaceConnectionFlags},
    ui::{format_vector, SpatialSpaceViewState},
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
    fn identifier() -> SpaceViewClassIdentifier {
        "3D".into()
    }

    fn display_name(&self) -> &'static str {
        "3D"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_3D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_3d::help_text(re_ui)
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<SpatialSpaceViewState>::default()
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

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn recommended_root_for_entities(
        &self,
        entities: &IntSet<EntityPath>,
        entity_db: &EntityDb,
    ) -> Option<EntityPath> {
        let common_ancestor = EntityPath::common_ancestor_of(entities.iter());

        // For 3D space view, the origin of the subspace defined by the common ancestor is usually
        // the best choice. However, if the subspace is defined by a pinhole, we should use its
        // parent.
        //
        // Also, if a ViewCoordinate3D is logged somewhere between the common ancestor and the
        // subspace origin, we use it as origin.
        SpatialTopology::access(entity_db.store_id(), |topo| {
            let common_ancestor_subspace = topo.subspace_for_entity(&common_ancestor);

            // Consider the case where the common ancestor might be in a 2D space that is connected
            // to a parent space. In this case, the parent space is the correct space.
            let subspace = if common_ancestor_subspace.supports_3d_content() {
                Some(common_ancestor_subspace)
            } else {
                topo.subspace_for_subspace_origin(common_ancestor_subspace.parent_space)
            };
            let subspace_origin = subspace.map(|subspace| subspace.origin.clone());

            // Find the first ViewCoordinates3d logged, walking up from the common ancestor to the
            // subspace origin.
            EntityPath::incremental_walk(subspace_origin.as_ref(), &common_ancestor)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .find(|path| {
                    subspace.is_some_and(|subspace| {
                        subspace
                            .heuristic_hints
                            .get(path)
                            .is_some_and(|hint| hint.contains(HeuristicHints::ViewCoordinates3d))
                    })
                })
                .or(subspace_origin)
        })
        .flatten()
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

            // All entities in the 3D space are visualizable + everything under pinholes.
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
                    // Entities _at_ pinholes are a special case: we display both 3D and 2D visualizers for them.
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
            Self::identifier(),
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
                    if !subspace.supports_3d_content() {
                        return None;
                    }

                    let mut pinhole_child_spaces = subspace
                        .child_spaces
                        .iter()
                        .filter(|child| {
                            topo.subspace_for_subspace_origin(child.hash()).map_or(
                                false,
                                |child_space| {
                                    child_space.connection_to_parent.is_connected_pinhole()
                                },
                            )
                        })
                        .peekable(); // Don't collect the iterator, we're only interested in 'any'-style operations.

                    // Empty space views are still of interest if any of the child spaces is connected via a pinhole.
                    if subspace.entities.is_empty() && pinhole_child_spaces.peek().is_none() {
                        return None;
                    }

                    // Creates space views at each view coordinates if there's any.
                    // (yes, we do so even if they're empty at the moment!)
                    //
                    // An exception to this rule is not to create a view there if this is already _also_ a subspace root.
                    // (e.g. this also has a camera or a `disconnect` logged there)
                    let mut origins = subspace
                        .heuristic_hints
                        .iter()
                        .filter(|(path, hint)| {
                            hint.contains(HeuristicHints::ViewCoordinates3d)
                                && !subspace.child_spaces.contains(path)
                        })
                        .map(|(path, _)| path.clone())
                        .collect::<Vec<_>>();

                    let path_not_covered_yet =
                        |e: &EntityPath| origins.iter().all(|origin| !e.starts_with(origin));

                    // If there's no view coordinates or there are still some entities not covered,
                    // create a view at the subspace origin.
                    if !origins.iter().contains(&subspace.origin)
                        && (indicated_entities
                            .intersection(&subspace.entities)
                            .any(path_not_covered_yet)
                            || pinhole_child_spaces.any(path_not_covered_yet))
                    {
                        origins.push(subspace.origin.clone());
                    }

                    Some(origins.into_iter().map(RecommendedSpaceView::new_subtree))
                })
                .flatten()
                .collect(),
        })
        .unwrap_or_default()
    }

    fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        state: &mut dyn SpaceViewState,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut re_entity_db::EntityPropertyMap,
    ) {
        let Ok(state) = state.downcast_mut::<SpatialSpaceViewState>() else {
            return;
        };
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
        state: &mut dyn SpaceViewState,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<SpatialSpaceViewState>()?;

        let scene_view_coordinates = ctx
            .entity_db
            .store()
            .query_latest_component::<ViewCoordinates>(space_origin, &ctx.current_query())
            .map(|c| c.value);

        ctx.re_ui
            .selection_grid(ui, "spatial_settings_ui")
            .show(ui, |ui| {
                state.default_size_ui(ctx, ui);

                ctx.re_ui
                    .grid_left_hand_label(ui, "Camera")
                    .on_hover_text("The virtual camera which controls what is shown on screen");
                ui.vertical(|ui| {
                    state.view_eye_ui(ctx.re_ui, ui, scene_view_coordinates);
                });
                ui.end_row();

                ctx.re_ui
                    .grid_left_hand_label(ui, "Coordinates")
                    .on_hover_text("The world coordinate system used for this view");
                ui.vertical(|ui| {
                    let up_description =
                        if let Some(scene_up) = scene_view_coordinates.and_then(|vc| vc.up()) {
                            format!("Scene up is {scene_up}")
                        } else {
                            "Scene up is unspecified".to_owned()
                        };
                    ui.label(up_description).on_hover_ui(|ui| {
                        re_ui::markdown_ui(
                            ui,
                            egui::Id::new("view_coordinates_tooltip"),
                            "Set with `rerun.ViewCoordinates`.",
                        );
                    });

                    if let Some(eye) = &state.state_3d.view_eye {
                        if let Some(eye_up) = eye.eye_up() {
                            ui.label(format!(
                                "Current camera-eye up-axis is {}",
                                format_vector(eye_up)
                            ));
                        }
                    }

                    ctx.re_ui
                        .checkbox(ui, &mut state.state_3d.show_axes, "Show origin axes")
                        .on_hover_text("Show X-Y-Z axes");
                    ctx.re_ui
                        .checkbox(ui, &mut state.state_3d.show_bbox, "Show bounding box")
                        .on_hover_text("Show the current scene bounding box");
                    ctx.re_ui
                        .checkbox(
                            ui,
                            &mut state.state_3d.show_accumulated_bbox,
                            "Show accumulated bounding box",
                        )
                        .on_hover_text("Show bounding box accumulated over all rendered frames");
                });
                ui.end_row();

                background_ui(ctx, space_view_id, ui);
                ui.end_row();

                state.bounding_box_ui(ctx, ui, SpatialSpaceViewKind::ThreeD);
                ui.end_row();
            });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<SpatialSpaceViewState>()?;

        state.bounding_boxes.update(&system_output.view_systems);
        state.scene_num_primitives = system_output
            .context_systems
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_3d::view_3d(ctx, ui, state, query, system_output)
    }
}

fn background_ui(ctx: &ViewerContext<'_>, space_view_id: SpaceViewId, ui: &mut egui::Ui) {
    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let (archetype, blueprint_path) =
        query_space_view_sub_archetype(space_view_id, blueprint_db, blueprint_query);

    let Background3D { color, mut kind } = archetype.unwrap_or_default();

    ctx.re_ui.grid_left_hand_label(ui, "Background");

    ui.vertical(|ui| {
        let kind_before = kind;
        egui::ComboBox::from_id_source("background")
            .selected_text(background_color_text(kind))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut kind,
                    Background3DKind::GradientDark,
                    background_color_text(Background3DKind::GradientDark),
                );
                ui.selectable_value(
                    &mut kind,
                    Background3DKind::GradientBright,
                    background_color_text(Background3DKind::GradientBright),
                );
                ui.selectable_value(
                    &mut kind,
                    Background3DKind::SolidColor,
                    background_color_text(Background3DKind::SolidColor),
                );
            });
        if kind_before != kind {
            ctx.save_blueprint_component(&blueprint_path, &kind);
        }

        if kind == Background3DKind::SolidColor {
            let current_color = color.unwrap_or(Background3D::DEFAULT_COLOR).into();
            let mut edit_color = current_color;
            egui::color_picker::color_edit_button_srgba(
                ui,
                &mut edit_color,
                egui::color_picker::Alpha::Opaque,
            );
            if edit_color != current_color {
                ctx.save_blueprint_component(
                    &blueprint_path,
                    &re_types::components::Color::from(edit_color),
                );
            }
        }
    });

    ui.end_row();
}

fn background_color_text(kind: Background3DKind) -> &'static str {
    match kind {
        Background3DKind::GradientDark => "Dark Gradient",
        Background3DKind::GradientBright => "Bright Gradient",
        Background3DKind::SolidColor => "Solid Color",
    }
}
