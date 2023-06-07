use re_viewer::external::{
    egui,
    re_log_types::EntityPath,
    re_space_view, re_ui,
    re_viewer_context::{
        HoverHighlight, Item, SelectionHighlight, SpaceViewClass, SpaceViewClassName, SpaceViewId,
        SpaceViewState, TypedScene, ViewerContext,
    },
};

use crate::color_coordinates_scene::SceneColorCoordinates;

#[derive(Default, Debug, PartialEq, Clone, Copy)]
enum ColorCoordinatesMode {
    #[default]
    Hs,
    Hv,
    Rg,
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

#[derive(Default)]
pub struct ColorCoordinatesSpaceViewState {
    // TODO(wumpf/jleibs): This should be part of the Blueprint so that it is serialized out.
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
    // TODO: document all of these.
    type State = ColorCoordinatesSpaceViewState;
    type SceneParts = SceneColorCoordinates;
    type Context = re_space_view::EmptySceneContext;
    type ScenePartData = ();

    fn name(&self) -> SpaceViewClassName {
        // Name and identifier of this space view.
        "Color Coordinates".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXT
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi, _state: &Self::State) -> egui::WidgetText {
        "A demo space view that shows colors as coordinates on a 2D plane.".into()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    /// Additional UI displayed when the space view is selected.
    fn selection_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        ui.vertical(|ui| {
            ui.label("Color coordinates mode");
            egui::ComboBox::from_id_source("color_coordinates_mode")
                .selected_text(state.mode.to_string())
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    let mut selectable_value = |ui: &mut egui::Ui, e| {
                        ui.selectable_value(&mut state.mode, e, e.to_string())
                    };
                    selectable_value(ui, ColorCoordinatesMode::Hs);
                    selectable_value(ui, ColorCoordinatesMode::Hv);
                    selectable_value(ui, ColorCoordinatesMode::Rg);
                });
        });
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene: &mut TypedScene<Self>,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| match state.mode {
            ColorCoordinatesMode::Hs => draw_color_space(
                ui,
                ctx,
                scene,
                |x, y| egui::ecolor::Hsva::new(x, y, 1.0, 1.0).into(),
                |c| {
                    let hsva: egui::ecolor::Hsva = c.into();
                    (hsva.h, hsva.s)
                },
            ),
            ColorCoordinatesMode::Hv => draw_color_space(
                ui,
                ctx,
                scene,
                |x, y| egui::ecolor::Hsva::new(x, 1.0, y, 1.0).into(),
                |c| {
                    let hsva: egui::ecolor::Hsva = c.into();
                    (hsva.h, hsva.v)
                },
            ),
            ColorCoordinatesMode::Rg => draw_color_space(
                ui,
                ctx,
                scene,
                |x, y| egui::ecolor::Rgba::from_rgb(x, y, 0.0).into(),
                |c| {
                    let rgba: egui::ecolor::Rgba = c.into();
                    (rgba.r(), rgba.g())
                },
            ),
        });
    }
}

// Draw a mesh for displaying the color space
// Inspired by https://github.com/emilk/egui/blob/0.22.0/crates/egui/src/widgets/color_picker.rs
fn draw_color_space(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    scene: &mut TypedScene<ColorCoordinatesSpaceView>,
    color_at: impl Fn(f32, f32) -> egui::Color32,
    position_at: impl Fn(egui::Color32) -> (f32, f32),
) -> egui::Response {
    // Number of vertices per dimension.
    // We need at least 6 for hues, and more for smooth 2D areas.
    // Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
    const N: u32 = 6 * 6;

    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());

    if ui.is_rect_visible(rect) {
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
        for (i, (instance, color)) in scene.parts.colors.iter().enumerate() {
            let highlight = scene
                .highlights
                .entity_highlight(instance.entity_path_hash)
                .index_highlight(instance.instance_key);

            let (x, y) = position_at(*color);
            let center = egui::pos2(
                egui::lerp(rect.left()..=rect.right(), x),
                egui::lerp(rect.bottom()..=rect.top(), y),
            );

            // Change color if this instance is selected.
            let (color, radius) = if highlight.hover != HoverHighlight::None {
                (ui.style().visuals.widgets.hovered.bg_fill, 4.0)
            } else if highlight.selection != SelectionHighlight::None {
                (ui.style().visuals.selection.bg_fill, 4.0)
            } else {
                (egui::Color32::BLACK, 2.0)
            };

            ui.painter()
                .add(egui::Shape::circle_filled(center, radius, color));

            let interact = ui.interact(
                egui::Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0)),
                ui.id().with(i),
                egui::Sense::click(),
            );

            // Update the globa selection state if the user interacts with this instance.
            if interact.hovered() {
                if let Some(instance) = instance.resolve(&ctx.store_db.entity_db) {
                    let item = Item::InstancePath(None, instance);

                    if interact.clicked() {
                        ctx.selection_state_mut()
                            .set_selection(std::iter::once(item.clone()));
                    }
                    ctx.selection_state_mut().set_hovered(std::iter::once(item));
                }
            }
        }
    }

    response
}
