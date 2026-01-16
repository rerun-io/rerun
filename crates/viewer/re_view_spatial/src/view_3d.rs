use ahash::HashSet;
use glam::Vec3;
use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{
    Background, EyeControls3D, LineGrid3D, SpatialInformation,
};
use re_sdk_types::blueprint::components::Eye3DKind;
use re_sdk_types::components::{LinearSpeed, Plane3D, Position3D, Vector3D};
use re_sdk_types::datatypes::Vec3D;
use re_sdk_types::view_coordinates::SignedAxis3;
use re_sdk_types::{Archetype as _, Component as _, View as _, ViewClassIdentifier, archetypes};
use re_tf::query_view_coordinates;
use re_ui::{Help, UiExt as _, list_item};
use re_view::view_property_ui;
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, PerVisualizer, PerVisualizerInViewClass,
    QueryContext, RecommendedView, RecommendedVisualizers, ViewClass, ViewClassExt as _,
    ViewClassRegistryError, ViewContext, ViewId, ViewQuery, ViewSpawnHeuristics, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewSystemIdentifier, ViewerContext,
    VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;
use smallvec::SmallVec;

use crate::contexts::register_spatial_contexts;
use crate::heuristics::IndicatedVisualizableEntities;
use crate::shared_fallbacks;
use crate::spatial_topology::{HeuristicHints, SpatialTopology, SubSpaceConnectionFlags};
use crate::ui::SpatialViewState;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{
    CamerasVisualizer, TransformAxes3DVisualizer, register_3d_spatial_visualizers,
};

#[derive(Default)]
pub struct SpatialView3D;

type ViewType = re_sdk_types::blueprint::views::Spatial3DView;

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

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        super::ui_3d::help(os)
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<SpatialViewState>::default()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry
            .register_fallback_provider(LineGrid3D::descriptor_color().component, |_| {
                re_sdk_types::components::Color::from_unmultiplied_rgba(128, 128, 128, 60)
            });

        system_registry.register_fallback_provider(
            LineGrid3D::descriptor_plane().component,
            |ctx| {
                const DEFAULT_PLANE: Plane3D = Plane3D::XY;

                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    return DEFAULT_PLANE;
                };

                view_state
                    .state_3d
                    .scene_view_coordinates
                    .and_then(|view_coordinates| view_coordinates.up())
                    .map_or(DEFAULT_PLANE, |up| Plane3D::new(up.as_vec3(), 0.0))
            },
        );

        system_registry
            .register_fallback_provider(LineGrid3D::descriptor_stroke_width().component, |_| {
                re_sdk_types::components::StrokeWidth::from(1.0)
            });

        system_registry.register_fallback_provider(
            Background::descriptor_kind().component,
            |ctx| match ctx.egui_ctx().theme() {
                egui::Theme::Dark => {
                    re_sdk_types::blueprint::components::BackgroundKind::GradientDark
                }
                egui::Theme::Light => {
                    re_sdk_types::blueprint::components::BackgroundKind::GradientBright
                }
            },
        );

        fn eye_property(ctx: &QueryContext<'_>) -> ViewProperty {
            ViewProperty::from_archetype::<EyeControls3D>(
                ctx.view_ctx.blueprint_db(),
                ctx.view_ctx.blueprint_query(),
                ctx.view_ctx.view_id,
            )
        }

        system_registry.register_fallback_provider(
            re_sdk_types::blueprint::archetypes::EyeControls3D::descriptor_speed().component,
            |ctx| {
                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    re_log::error_once!(
                        "Fallback for `LinearSpeed` queried on 3D view outside the context of a spatial view."
                    );
                    return 1.0.into();
                };

                let eye = eye_property(ctx);


                let Ok(kind) = eye.component_or_fallback::<Eye3DKind>(ctx.view_ctx, EyeControls3D::descriptor_kind().component) else {
                    return 1.0.into();
                };

                let speed = match kind {
                    Eye3DKind::FirstPerson => {
                        let l = view_state.bounding_boxes.current.size().length() as f64;
                        if l.is_finite() {
                            0.1 * l
                        } else {
                            1.0
                        }
                    },
                    Eye3DKind::Orbital => {
                        let Ok(position) = eye.component_or_fallback::<Position3D>(ctx.view_ctx, EyeControls3D::descriptor_position().component) else {
                            return 1.0.into();
                        };
                        let Ok(look_target) = eye.component_or_fallback::<Position3D>(ctx.view_ctx, EyeControls3D::descriptor_look_target().component) else {
                            return 1.0.into();
                        };

                        // Use the orbit radius for speed.
                        Vec3::from_array(position.0.0).as_dvec3().distance(Vec3::from_array(look_target.0.0).as_dvec3())
                    },
                };

                LinearSpeed::from(speed)
            },
        );

        system_registry.register_fallback_provider(
            re_sdk_types::blueprint::archetypes::EyeControls3D::descriptor_look_target().component,
            |ctx| {
                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    re_log::error_once!(
                        "Fallback for `Position3D` queried on 3D view outside the context of a spatial view."
                    );
                    return Position3D::ZERO;
                };
                let center = view_state.bounding_boxes.current.center();

                if !center.is_finite() {
                    return Position3D::ZERO;
                }

                Position3D::from(center)
            },
        );

        system_registry.register_fallback_provider(
            re_sdk_types::blueprint::archetypes::EyeControls3D::descriptor_position().component,
            |ctx| {
                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    re_log::error_once!(
                        "Fallback for `Position3D` queried on 3D view outside the context of a spatial view."
                    );
                    return Position3D::ZERO;
                };
                let mut center = view_state.bounding_boxes.current.center();

                if !center.is_finite() {
                    center = Vec3::ZERO;
                }

                let mut radius = 1.5 * view_state.bounding_boxes.current.half_size().length();
                if !radius.is_finite() || radius == 0.0 {
                    radius = 1.0;
                }


                let scene_view_coordinates =
                    view_state.state_3d.scene_view_coordinates.unwrap_or_default();

                let scene_right = scene_view_coordinates
                    .right()
                    .unwrap_or(SignedAxis3::POSITIVE_X);
                let scene_forward = scene_view_coordinates
                    .forward()
                    .unwrap_or(SignedAxis3::POSITIVE_Y);
                let scene_up = scene_view_coordinates
                    .up()
                    .unwrap_or(SignedAxis3::POSITIVE_Z);

                let eye_up: glam::Vec3 = scene_up.into();

                let eye_dir = {
                    // Make sure that the right of the scene is to the right for
                    // the default camera view.
                    let right = scene_right.into();
                    let fwd = eye_up.cross(right);
                    0.75 * fwd + 0.25 * right - 0.25 * eye_up
                };

                let eye_dir = eye_dir.try_normalize().unwrap_or_else(|| scene_forward.into());

                let eye_pos = center - radius * eye_dir;

                Position3D::from(eye_pos)
            },
        );

        system_registry.register_fallback_provider(
            re_sdk_types::blueprint::archetypes::EyeControls3D::descriptor_eye_up().component,
            |ctx| {
                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    re_log::error_once!(
                        "Fallback for `Vector3D` queried on 3D view outside the context of a spatial view."
                    );
                    return Vector3D(Vec3D::new(0.0, 0.0, 1.0));
                };

                let scene_view_coordinates =
                    view_state.state_3d.scene_view_coordinates.unwrap_or_default();

                let scene_up = scene_view_coordinates
                    .up()
                    .unwrap_or(SignedAxis3::POSITIVE_Z);

                let eye_up = glam::Vec3::from(scene_up).normalize_or(Vec3::Z);

                Vector3D(Vec3D::new(eye_up.x, eye_up.y, eye_up.z))
            },
        );

        shared_fallbacks::register_fallbacks(system_registry);

        // Ensure spatial topology is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();

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

    fn recommended_origin_for_entities(
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

    /// Choose the default visualizers to enable for this entity.
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizerInViewClass<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> RecommendedVisualizers {
        let axes_viz = TransformAxes3DVisualizer::identifier();
        let camera_viz = CamerasVisualizer::identifier();

        let visualizable: HashSet<&ViewSystemIdentifier> = visualizable_entities_per_visualizer
            .iter()
            .filter_map(|(visualizer, ents)| ents.contains_key(entity_path).then_some(visualizer))
            .collect();

        let indicated: HashSet<&ViewSystemIdentifier> = indicated_entities_per_visualizer
            .iter()
            .filter_map(|(visualizer, ents)| {
                if ents.contains(entity_path) {
                    Some(visualizer)
                } else {
                    None
                }
            })
            .collect();

        // Start with all the entities which are both indicated and visualizable.
        let mut enabled_visualizers: SmallVec<[ViewSystemIdentifier; 4]> = indicated
            .intersection(&visualizable)
            .copied()
            .copied()
            .collect();

        // Arrow visualizer is not enabled yet but we could…
        if !enabled_visualizers.contains(&axes_viz) && visualizable.contains(&axes_viz) {
            // …if we already have the [`CamerasVisualizer`] active.
            if enabled_visualizers.contains(&camera_viz) {
                enabled_visualizers.push(axes_viz);
            }
        }

        RecommendedVisualizers::default_many(enabled_visualizers)
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        let IndicatedVisualizableEntities {
            indicated_entities,
            excluded_entities,
        } = IndicatedVisualizableEntities::new(
            ctx,
            Self::identifier(),
            SpatialViewKind::ThreeD,
            include_entity,
            |indicated_entities| {
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
                    if let Some(components) = engine.store().all_components_for_entity(path)
                        && components.into_iter().any(|component| {
                            archetypes::Pinhole::all_components().iter().any(|c| c.component == component)
                            // TODO(#2663): Note that the view coordinates component may be logged by different archetypes.
                                || component
                                    == archetypes::ViewCoordinates::descriptor_xyz().component
                        })
                    {
                        indicated_entities.insert(path.clone());
                    }
                });
            },
        );

        // Spawn a view at each subspace that has any potential 3D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(ctx.store_id(), |topo| {
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
                                topo.subspace_for_subspace_origin(child.hash()).is_some_and(
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

                        Some(origins.into_iter().map(RecommendedView::new_subtree).map(
                            |mut subtree| {
                                // Since we don't track the transform frames created by explicit
                                // coordinate frames, we can't make assumptions about the tree if
                                // there are any explicit coordinate frames.
                                if !topo.has_explicit_coordinate_frame() {
                                    subtree.exclude_entities(&excluded_entities);
                                }

                                subtree
                            },
                        ))
                    })
                    .flatten(),
            )
        })
        .unwrap_or_else(ViewSpawnHeuristics::empty)
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

        let scene_view_coordinates =
            query_view_coordinates(space_origin, ctx.recording(), &ctx.current_query());

        // TODO(andreas): list_item'ify the rest
        ui.selection_grid("spatial_settings_ui").show(ui, |ui| {
            ui.grid_left_hand_label("Camera")
                .on_hover_text("The virtual camera which controls what is shown on screen");
            ui.vertical(|ui| {
                state.view_eye_ui(ui, ctx, view_id);
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
            });
            ui.end_row();

            state.bounding_box_ui(ui, SpatialViewKind::ThreeD);

            #[cfg(debug_assertions)]
            ui.re_checkbox(&mut state.state_3d.show_smoothed_bbox, "Smoothed bbox");
        });

        re_ui::list_item::list_item_scope(ui, "spatial_view3d_selection_ui", |ui| {
            let view_ctx = self.view_context(ctx, view_id, state, space_origin);
            view_property_ui::<SpatialInformation>(&view_ctx, ui);
            view_property_ui::<EyeControls3D>(&view_ctx, ui);
            view_property_ui::<Background>(&view_ctx, ui);
            view_property_ui_grid3d(&view_ctx, ui);
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
fn view_property_ui_grid3d(ctx: &ViewContext<'_>, ui: &mut egui::Ui) {
    let property = ViewProperty::from_archetype::<LineGrid3D>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    );
    let reflection = ctx.viewer_ctx.reflection();
    let Some(reflection) = reflection.archetypes.get(&property.archetype_name) else {
        ui.error_label(format!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        ));
        return;
    };

    let query_ctx = property.query_context(ctx);
    let sub_prop_ui = |ui: &mut egui::Ui| {
        for field in &reflection.fields {
            // TODO(#1611): The color picker for the color component doesn't show alpha values so far since alpha is almost never supported.
            // Here however, we need that alpha color picker!
            if field.component_type == re_sdk_types::components::Color::name() {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|ui| {
                        let Ok(color) = property
                            .component_or_fallback::<re_sdk_types::components::Color>(
                                ctx,
                                LineGrid3D::descriptor_color().component,
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
                            let color = re_sdk_types::components::Color::from(edit_color);
                            property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &LineGrid3D::descriptor_color(),
                                &color,
                            );
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
