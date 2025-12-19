#![expect(clippy::disallowed_methods)] // It's just an example

use rerun::external::egui;
use rerun::external::re_data_ui::{DataUi, item_ui};
use rerun::external::re_entity_db::InstancePath;
use rerun::external::re_log_types::EntityPath;
use rerun::external::re_sdk_types::ViewClassIdentifier;
use rerun::external::re_ui::{self, Help};
use rerun::external::re_viewer_context::{
    HoverHighlight, IdentifiedViewSystem as _, IndicatedEntities, Item, PerVisualizer,
    PerVisualizerInViewClass, RecommendedVisualizers, SelectionHighlight, SystemExecutionOutput,
    UiLayout, ViewClass, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemRegistrator, ViewerContext, VisualizableEntities,
};

use crate::points3d_color_visualizer::{ColorWithInstance, Points3DColorVisualizer};

/// The different modes for displaying color coordinates in the custom view.
#[derive(Default, Debug, PartialEq, Clone, Copy)]
enum ColorCoordinatesMode {
    #[default]
    Hs,
    Hv,
    Rg,
}

impl ColorCoordinatesMode {
    pub const ALL: [ColorCoordinatesMode; 3] = [
        ColorCoordinatesMode::Hs,
        ColorCoordinatesMode::Hv,
        ColorCoordinatesMode::Rg,
    ];
}

impl std::fmt::Display for ColorCoordinatesMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorCoordinatesMode::Hs => "Hue/Saturation".fmt(f),
            ColorCoordinatesMode::Hv => "Hue/Value".fmt(f),
            ColorCoordinatesMode::Rg => "Red/Green".fmt(f),
        }
    }
}

/// View state for the custom view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct ColorCoordinatesViewState {
    // TODO(wumpf, jleibs): This should be part of the Blueprint so that it is serialized out.
    //                      but right now there is no way of doing that.
    mode: ColorCoordinatesMode,
}

impl ViewState for ColorCoordinatesViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct ColorCoordinatesView;

impl ViewClass for ColorCoordinatesView {
    // State type as described above.

    fn identifier() -> ViewClassIdentifier {
        "ColorCoordinates".into()
    }

    fn display_name(&self) -> &'static str {
        "Color coordinates"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_GENERIC
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Color coordinates view")
            .markdown("A demo view that shows colors as coordinates on a 2D plane.")
    }

    /// Register all systems (contexts & parts) that the view needs.
    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<Points3DColorVisualizer>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<ColorCoordinatesViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        // By default spawn a single view at the root if there's anything the visualizer may be able to show.
        if ctx
            .visualizable_entities_per_visualizer
            .get(&Points3DColorVisualizer::identifier())
            .is_some_and(|entities| entities.keys().any(include_entity))
        {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    /// Make the viewer use the `ColorCoordinatesVisualizerSystem` by default.
    ///
    /// The default implementation of `choose_default_visualizers` activates visualizers only
    /// if the respective indicator is present.
    /// We want to enable the visualizer here though for any visualizable entity instead!
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizerInViewClass<VisualizableEntities>,
        _indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> RecommendedVisualizers {
        if visualizable_entities_per_visualizer
            .get(&Points3DColorVisualizer::identifier())
            .is_some_and(|entities| entities.contains_key(entity_path))
        {
            RecommendedVisualizers::default(Points3DColorVisualizer::identifier())
        } else {
            RecommendedVisualizers::empty()
        }
    }

    /// Additional UI displayed when the view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<ColorCoordinatesViewState>()?;

        ui.horizontal(|ui| {
            ui.label("Coordinates mode");
            egui::ComboBox::from_id_salt("color_coordinates_mode")
                .selected_text(state.mode.to_string())
                .show_ui(ui, |ui| {
                    for mode in &ColorCoordinatesMode::ALL {
                        ui.selectable_value(&mut state.mode, *mode, mode.to_string());
                    }
                });
        });

        Ok(())
    }

    /// The contents of the View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,

        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let colors = system_output
            .view_systems
            .get::<Points3DColorVisualizer>()?;
        let state = state.downcast_mut::<ColorCoordinatesViewState>()?;

        egui::Frame::default().show(ui, |ui| {
            let color_at = match state.mode {
                ColorCoordinatesMode::Hs => |x, y| egui::ecolor::Hsva::new(x, y, 1.0, 1.0).into(),
                ColorCoordinatesMode::Hv => |x, y| egui::ecolor::Hsva::new(x, 1.0, y, 1.0).into(),
                ColorCoordinatesMode::Rg => |x, y| egui::ecolor::Rgba::from_rgb(x, y, 0.0).into(),
            };
            let position_at = match state.mode {
                ColorCoordinatesMode::Hs => |c: egui::Color32| {
                    let hsva = egui::ecolor::Hsva::from(c);
                    (hsva.h, hsva.s)
                },
                ColorCoordinatesMode::Hv => |c: egui::Color32| {
                    let hsva = egui::ecolor::Hsva::from(c);
                    (hsva.h, hsva.v)
                },
                ColorCoordinatesMode::Rg => |c: egui::Color32| {
                    let rgba = egui::ecolor::Rgba::from(c);
                    (rgba.r(), rgba.g())
                },
            };
            color_space_ui(ui, ctx, colors, query, color_at, position_at);
        });
        Ok(())
    }
}

// Draw a mesh for displaying the color space
// Inspired by https://github.com/emilk/egui/blob/0.22.0/crates/egui/src/widgets/color_picker.rs
fn color_space_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    colors: &Points3DColorVisualizer,
    query: &ViewQuery<'_>,
    color_at: impl Fn(f32, f32) -> egui::Color32,
    position_at: impl Fn(egui::Color32) -> (f32, f32),
) -> egui::Response {
    // Number of vertices per dimension.
    // We need at least 6 for hues, and more for smooth 2D areas.
    // Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
    const N: u32 = 6 * 6;

    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Background space.
    let mut mesh = egui::Mesh::default();
    for xi in 0..=N {
        for yi in 0..=N {
            let xt = xi as f32 / (N as f32);
            let yt = yi as f32 / (N as f32);
            let color = color_at(xt, yt);
            let x = egui::lerp(rect.left()..=rect.right(), xt);
            let y = egui::lerp(rect.bottom()..=rect.top(), yt);
            mesh.colored_vertex(egui::pos2(x, y), color);

            if xi < N && yi < N {
                let x_offset = 1;
                let y_offset = N + 1;
                let tl = yi * y_offset + xi;
                mesh.add_triangle(tl, tl + x_offset, tl + y_offset);
                mesh.add_triangle(tl + x_offset, tl + y_offset, tl + y_offset + x_offset);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));

    // Circles for the colors in the scene.
    let mut hovering_any_point = false;
    for (ent_path, colors) in &colors.colors {
        let ent_highlight = query.highlights.entity_highlight(ent_path.hash());
        for ColorWithInstance { instance, color } in colors {
            let highlight = ent_highlight.index_highlight(*instance);

            let (x, y) = position_at(*color);
            let center = egui::pos2(
                egui::lerp(rect.left()..=rect.right(), x),
                egui::lerp(rect.bottom()..=rect.top(), y),
            );

            // Change color & size depending on whether this instance is selected.
            let (color, radius) = match (
                highlight.hover,
                highlight.selection != SelectionHighlight::None,
            ) {
                (HoverHighlight::None, false) => (ui.style().visuals.extreme_bg_color, 2.0),
                (HoverHighlight::None, true) => (ui.style().visuals.selection.bg_fill, 8.0),
                (HoverHighlight::Hovered, ..) => (ui.style().visuals.widgets.hovered.bg_fill, 8.0),
            };

            ui.painter()
                .add(egui::Shape::circle_filled(center, radius, color));

            let interact = ui.interact(
                egui::Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0)),
                ui.id().with(("circle", &ent_path, instance)),
                egui::Sense::click(),
            );

            // Update the global selection state if the user interacts with a point and show hover ui for the entire keypoint.
            let instance = InstancePath::instance(ent_path.clone(), *instance);
            let interact = interact.on_hover_ui_at_pointer(|ui| {
                item_ui::instance_path_button(
                    ctx,
                    &ctx.current_query(),
                    ctx.recording(),
                    ui,
                    Some(query.view_id),
                    &instance,
                );
                instance.data_ui(
                    ctx,
                    ui,
                    UiLayout::Tooltip,
                    &ctx.current_query(),
                    ctx.recording(),
                );

                hovering_any_point = true;
            });
            ctx.handle_select_hover_drag_interactions(
                &interact,
                Item::DataResult(query.view_id, instance),
                false,
            );
        }
    }

    // If no point was selected, then select the view.
    if !hovering_any_point {
        ctx.handle_select_hover_drag_interactions(&response, Item::View(query.view_id), false);
    }

    response
}
