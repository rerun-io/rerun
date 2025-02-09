use crate::color_coordinates_visualizer_system::{ColorWithInstance, InstanceColorSystem};
use re_viewer::external::re_ui::Help;
use re_viewer::external::{
    egui,
    re_data_ui::{item_ui, DataUi},
    re_entity_db::InstancePath,
    re_log_types::EntityPath,
    re_types::ViewClassIdentifier,
    re_ui,
    re_viewer_context::{
        HoverHighlight, IdentifiedViewSystem as _, Item, SelectionHighlight, SystemExecutionOutput,
        UiLayout, ViewClass, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
        ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
        ViewSystemRegistrator, ViewerContext,
    },
};

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

    fn help(&self, _egui_ctx: &egui::Context) -> Help<'_> {
        Help::new("Color coordinates view")
            .markdown("A demo view that shows colors as coordinates on a 2D plane.")
    }

    /// Register all systems (contexts & parts) that the view needs.
    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<InstanceColorSystem>()
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

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> ViewSpawnHeuristics {
        // By default spawn a single view at the root if there's anything the visualizer may be able to show.
        if ctx
            .maybe_visualizable_entities_per_visualizer
            .get(&InstanceColorSystem::identifier())
            .map_or(true, |entities| entities.is_empty())
        {
            ViewSpawnHeuristics::default()
        } else {
            ViewSpawnHeuristics::root()
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
        let colors = system_output.view_systems.get::<InstanceColorSystem>()?;
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
    colors: &InstanceColorSystem,
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
                (HoverHighlight::None, false) => (egui::Color32::BLACK, 2.0),
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
            });
            ctx.handle_select_hover_drag_interactions(
                &interact,
                Item::DataResult(query.view_id, instance),
                false,
            );
        }
    }

    response
}
