use re_viewer::external::{
    egui,
    re_data_ui::{item_ui, DataUi},
    re_entity_db::{EntityProperties, InstancePath},
    re_log_types::EntityPath,
    re_ui,
    re_viewer_context::{
        HoverHighlight, Item, SelectionHighlight, SpaceViewClass, SpaceViewClassLayoutPriority,
        SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError,
        SpaceViewSystemRegistrator, SystemExecutionOutput, UiVerbosity, ViewQuery, ViewerContext,
    },
};

use crate::color_coordinates_visualizer_system::{ColorWithInstanceKey, InstanceColorSystem};

/// The different modes for displaying color coordinates in the custom space view.
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

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct ColorCoordinatesSpaceViewState {
    // TODO(wumpf, jleibs): This should be part of the Blueprint so that it is serialized out.
    //                      but right now there is no way of doing that.
    mode: ColorCoordinatesMode,
}

impl SpaceViewState for ColorCoordinatesSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct ColorCoordinatesSpaceView;

impl SpaceViewClass for ColorCoordinatesSpaceView {
    // State type as described above.
    type State = ColorCoordinatesSpaceViewState;

    const IDENTIFIER: &'static str = "Color Coordinates";
    const DISPLAY_NAME: &'static str = "Color Coordinates";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_GENERIC
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "A demo space view that shows colors as coordinates on a 2D plane.".into()
    }

    /// Register all systems (contexts & parts) that the space view needs.
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<InstanceColorSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        Default::default()
    }

    /// Additional UI displayed when the space view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        ui.horizontal(|ui| {
            ui.label("Coordinates mode");
            egui::ComboBox::from_id_source("color_coordinates_mode")
                .selected_text(state.mode.to_string())
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    for mode in &ColorCoordinatesMode::ALL {
                        ui.selectable_value(&mut state.mode, *mode, mode.to_string());
                    }
                });
        });
    }

    /// The contents of the Space View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let colors = system_output.view_systems.get::<InstanceColorSystem>()?;

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
        for ColorWithInstanceKey {
            instance_key,
            color,
        } in colors
        {
            let highlight = ent_highlight.index_highlight(*instance_key);

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
                ui.id().with(("circle", &ent_path, instance_key)),
                egui::Sense::click(),
            );

            // Update the global selection state if the user interacts with a point and show hover ui for the entire keypoint.
            let instance = InstancePath::instance(ent_path.clone(), *instance_key);
            let interact = interact.on_hover_ui_at_pointer(|ui| {
                item_ui::instance_path_button(
                    ctx,
                    &ctx.current_query(),
                    ctx.entity_db.store(),
                    ui,
                    Some(query.space_view_id),
                    &instance,
                );
                instance.data_ui(
                    ctx,
                    ui,
                    UiVerbosity::Reduced,
                    &ctx.current_query(),
                    ctx.entity_db.store(),
                );
            });
            item_ui::select_hovered_on_click(
                ctx,
                &interact,
                Item::InstancePath(Some(query.space_view_id), instance),
            );
        }
    }

    response
}
