use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::blueprint::archetypes::LineGrid3D;
use re_types::{
    blueprint::archetypes::Background, components::ViewCoordinates, Component, View,
    ViewClassIdentifier,
};
use re_ui::{list_item, UiExt as _};
use re_view::view_property_ui;
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, IndicatedEntities, PerVisualizer, RecommendedView,
    SmallVisualizerSet, ViewClass, ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewSystemIdentifier, ViewerContext,
    VisualizableEntities, VisualizableFilterContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::visualizers::{AxisLengthDetector, CamerasVisualizer, Transform3DArrowsVisualizer};
use crate::{
    contexts::register_spatial_contexts,
    heuristics::default_visualized_entities_for_visualizer_kind,
    spatial_topology::{HeuristicHints, SpatialTopology, SubSpaceConnectionFlags},
    ui::{format_vector, SpatialViewState},
    view_kind::SpatialViewKind,
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
pub struct SpatialView3D;

type ViewType = re_types::blueprint::views::Spatial3DView;

impl ViewClass for SpatialView3D {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "3D"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_3D
    }

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        super::ui_3d::help_markdown(egui_ctx)
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<SpatialViewState>::default()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        // Ensure spatial topology is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();
        crate::transform_component_tracker::TransformComponentTrackerStoreSubscriber::subscription_handle();

        register_spatial_contexts(system_registry)?;
        register_3d_spatial_visualizers(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    fn supports_visible_time_range(&self) -> bool {
        true
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::High
    }

    fn recommended_root_for_entities(
        &self,
        entities: &IntSet<EntityPath>,
        entity_db: &EntityDb,
    ) -> Option<EntityPath> {
        let common_ancestor = EntityPath::common_ancestor_of(entities.iter());

        // For 3D view, the origin of the subspace defined by the common ancestor is usually
        // the best choice. However, if the subspace is defined by a pinhole, we should use its
        // parent.
        //
        // Also, if a ViewCoordinate3D is logged somewhere between the common ancestor and the
        // subspace origin, we use it as origin.
        SpatialTopology::access(&entity_db.store_id(), |topo| {
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

        let context = SpatialTopology::access(&entity_db.store_id(), |topo| {
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

    /// Choose the default visualizers to enable for this entity.
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        applicable_entities_per_visualizer: &PerVisualizer<ApplicableEntities>,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> SmallVisualizerSet {
        let arrows_viz = Transform3DArrowsVisualizer::identifier();
        let axis_detector = AxisLengthDetector::identifier();
        let camera_viz = CamerasVisualizer::identifier();

        let applicable: HashSet<&ViewSystemIdentifier> = applicable_entities_per_visualizer
            .iter()
            .filter_map(|(visualizer, ents)| {
                if ents.contains(entity_path) {
                    Some(visualizer)
                } else {
                    None
                }
            })
            .collect();

        let visualizable: HashSet<&ViewSystemIdentifier> = visualizable_entities_per_visualizer
            .iter()
            .filter_map(|(visualizer, ents)| {
                if ents.contains(entity_path) {
                    Some(visualizer)
                } else {
                    None
                }
            })
            .collect();

        // We never want to consider `Transform3DArrows` as directly indicated since it uses the
        // the Transform3D archetype. This is often used to transform other 3D primitives, where
        // it might be annoying to always have the arrows show up.
        let indicated: HashSet<&ViewSystemIdentifier> = indicated_entities_per_visualizer
            .iter()
            .filter_map(|(visualizer, ents)| {
                if visualizer != &arrows_viz && ents.contains(entity_path) {
                    Some(visualizer)
                } else {
                    None
                }
            })
            .collect();

        // Start with all the entities which are both indicated and visualizable.
        let mut enabled_visualizers: SmallVisualizerSet = indicated
            .intersection(&visualizable)
            .copied()
            .copied()
            .collect();

        // Arrow visualizer is not enabled yet but we could…
        if !enabled_visualizers.contains(&arrows_viz) && visualizable.contains(&arrows_viz) {
            // … then we enable it if either:
            // - If someone set an axis_length explicitly, so [`AxisLengthDetector`] is applicable.
            // - If we already have the [`CamerasVisualizer`] active.
            if applicable.contains(&axis_detector) || enabled_visualizers.contains(&camera_viz) {
                enabled_visualizers.push(arrows_viz);
            }
        }

        enabled_visualizers
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        let mut indicated_entities = default_visualized_entities_for_visualizer_kind(
            ctx,
            Self::identifier(),
            SpatialViewKind::ThreeD,
        );

        // ViewCoordinates is a strong indicator that a 3D view is needed.
        // Note that if the root has `ViewCoordinates`, this will stop the root splitting heuristic
        // from splitting the root space into several subspaces.
        //
        // TODO(andreas):
        // It's tempting to add a visualizer for view coordinates so that it's already picked up via `entities_with_indicator_for_visualizer_kind`.
        // Is there a nicer way for this or do we want a visualizer for view coordinates anyways?
        // There's also a strong argument to be made that ViewCoordinates implies a 3D space, thus changing the SpacialTopology accordingly!
        let engine = ctx.recording_engine();
        ctx.recording().tree().visit_children_recursively(|path| {
            if engine
                .store()
                .entity_has_component(path, &ViewCoordinates::name())
            {
                indicated_entities.insert(path.clone());
            }
        });

        // Spawn a view at each subspace that has any potential 3D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(&ctx.recording_id(), |topo| {
            ViewSpawnHeuristics::new(
                topo.iter_subspaces()
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
                                        child_space
                                            .connection_to_parent
                                            .contains(SubSpaceConnectionFlags::Pinhole)
                                    },
                                )
                            })
                            .peekable(); // Don't collect the iterator, we're only interested in 'any'-style operations.

                        // Empty views are still of interest if any of the child spaces is connected via a pinhole.
                        if subspace.entities.is_empty() && pinhole_child_spaces.peek().is_none() {
                            return None;
                        }

                        // Creates views at each view coordinates if there's any.
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

                        Some(origins.into_iter().map(RecommendedView::new_subtree))
                    })
                    .flatten(),
            )
        })
        .unwrap_or_default()
    }

    fn selection_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<SpatialViewState>()?;

        let scene_view_coordinates = ctx
            .recording()
            .latest_at_component::<ViewCoordinates>(space_origin, &ctx.current_query())
            .map(|(_index, c)| c);

        // TODO(andreas): list_item'ify the rest
        ui.selection_grid("spatial_settings_ui").show(ui, |ui| {
            ui.grid_left_hand_label("Camera")
                .on_hover_text("The virtual camera which controls what is shown on screen");
            ui.vertical(|ui| {
                state.view_eye_ui(ui, scene_view_coordinates);
            });
            ui.end_row();

            ui.grid_left_hand_label("Coordinates")
                .on_hover_text("The world coordinate system used for this view");
            ui.vertical(|ui| {
                let up_description =
                    if let Some(scene_up) = scene_view_coordinates.and_then(|vc| vc.up()) {
                        format!("Scene up is {scene_up}")
                    } else {
                        "Scene up is unspecified".to_owned()
                    };
                ui.label(up_description).on_hover_ui(|ui| {
                    ui.markdown_ui("Set with `rerun.ViewCoordinates`.");
                });

                if let Some(eye) = &state.state_3d.view_eye {
                    if let Some(eye_up) = eye.eye_up() {
                        ui.label(format!(
                            "Current camera-eye up-axis is {}",
                            format_vector(eye_up)
                        ));
                    }
                }

                ui.re_checkbox(&mut state.state_3d.show_axes, "Show origin axes")
                    .on_hover_text("Show X-Y-Z axes");
                ui.re_checkbox(&mut state.state_3d.show_bbox, "Show bounding box")
                    .on_hover_text("Show the current scene bounding box");
                ui.re_checkbox(
                    &mut state.state_3d.show_smoothed_bbox,
                    "Show smoothed bounding box",
                )
                .on_hover_text("Show a smoothed bounding box used for some heuristics");
            });
            ui.end_row();

            state.bounding_box_ui(ui, SpatialViewKind::ThreeD);
        });

        re_ui::list_item::list_item_scope(ui, "spatial_view3d_selection_ui", |ui| {
            view_property_ui::<Background>(ctx, ui, view_id, self, state);
            view_property_ui_grid3d(ctx, ui, view_id, self, state);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,

        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<SpatialViewState>()?;
        state.update_frame_statistics(ui, &system_output, SpatialViewKind::ThreeD);

        self.view_3d(ctx, ui, state, query, system_output)
    }
}

// The generic ui (via `view_property_ui::<Background>(ctx, ui, view_id, self, state);`)
// is suitable for the most part. However, as of writing the alpha color picker doesn't handle alpha
// which we need here.
fn view_property_ui_grid3d(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    fallback_provider: &dyn re_viewer_context::ComponentFallbackProvider,
    view_state: &dyn ViewState,
) {
    let property = ViewProperty::from_archetype::<LineGrid3D>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        view_id,
    );
    let Some(reflection) = ctx.reflection.archetypes.get(&property.archetype_name) else {
        ui.error_label(format!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        ));
        return;
    };

    let query_ctx = property.query_context(ctx, view_state);
    let sub_prop_ui = |ui: &mut egui::Ui| {
        for field in &reflection.fields {
            // TODO(#1611): The color picker for the color component doesn't show alpha values so far since alpha is almost never supported.
            // Here however, we need that alpha color picker!
            if field.component_name == re_types::components::Color::name() {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|ui| {
                        let Ok(color) = property
                            .component_or_fallback::<re_types::components::Color>(
                                ctx,
                                fallback_provider,
                                view_state,
                            )
                        else {
                            ui.error_label("Failed to query color component");
                            return;
                        };
                        let mut edit_color = egui::Color32::from(*color);
                        if egui::color_picker::color_edit_button_srgba(
                            ui,
                            &mut edit_color,
                            egui::color_picker::Alpha::OnlyBlend,
                        )
                        .changed()
                        {
                            let color = re_types::components::Color::from(edit_color);
                            property.save_blueprint_component(ctx, &[color]);
                        }
                    },
                    None, // No multiline editor.
                );
            } else {
                re_view::view_property_component_ui(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    fallback_provider,
                );
            }
        }
    };

    ui.list_item()
        .interactive(false)
        .show_hierarchical_with_children(
            ui,
            ui.make_persistent_id(property.archetype_name.full_name()),
            true,
            list_item::LabelContent::new(reflection.display_name),
            sub_prop_ui,
        );
}
