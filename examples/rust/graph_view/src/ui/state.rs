use std::collections::HashMap;

use re_format::format_f32;
use re_viewer::external::{
    egui::{self, emath},
    re_ui::UiExt,
    re_viewer_context::SpaceViewState,
};

use crate::{graph::NodeIndex, layout::LayoutProvider};

use super::bounding_rect_from_iter;

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
pub(crate) struct GraphSpaceViewState {
    pub world_to_view: emath::TSTransform,
    pub clip_rect_window: egui::Rect,

    // Debug information
    pub show_debug: bool,

    /// Positions of the nodes in world space.
    pub layout: Option<HashMap<NodeIndex, egui::Rect>>,
    pub layout_provider: LayoutProvider,
}

impl Default for GraphSpaceViewState {
    fn default() -> Self {
        Self {
            world_to_view: Default::default(),
            clip_rect_window: egui::Rect::NOTHING,
            show_debug: Default::default(),
            layout: Default::default(),
            layout_provider: LayoutProvider::new_fruchterman_reingold(),
        }
    }
}

impl GraphSpaceViewState {
    pub fn fit_to_screen(&mut self, bounding_rect: egui::Rect, available_size: egui::Vec2) {
        // Compute the scale factor to fit the bounding rectangle into the available screen size.
        let scale_x = available_size.x / bounding_rect.width();
        let scale_y = available_size.y / bounding_rect.height();

        // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
        let scale = scale_x.min(scale_y).min(1.0);

        // Compute the translation to center the bounding rect in the screen.
        let center_screen = egui::Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
        let center_world = bounding_rect.center().to_vec2();

        // Set the transformation to scale and then translate to center.
        self.world_to_view =
            emath::TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
                * emath::TSTransform::from_scaling(scale);
    }

    pub fn bounding_box_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(layout) = &self.layout {
            ui.grid_left_hand_label("Bounding box")
                .on_hover_text("The bounding box encompassing all Entities in the view right now");
            ui.vertical(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                if let Some(egui::Rect { min, max }) = bounding_rect_from_iter(layout.values()) {
                    ui.label(format!("x [{} - {}]", format_f32(min.x), format_f32(max.x),));
                    ui.label(format!("y [{} - {}]", format_f32(min.y), format_f32(max.y),));
                }
            });
            ui.end_row();
            if ui
                .button("Fit to screen")
                .on_hover_text("Fit the bounding box to the screen")
                .clicked()
            {
                if let Some(bounding_rect) = bounding_rect_from_iter(layout.values()) {
                    self.fit_to_screen(bounding_rect, self.clip_rect_window.size());
                }
            }
            ui.end_row();
        }
    }

    pub fn debug_ui(&mut self, ui: &mut egui::Ui) {
        ui.re_checkbox(&mut self.show_debug, "Show debug information")
            .on_hover_text("Shows debug information for the current graph");
        ui.end_row();
    }

    pub fn layout_provider_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Layout algorithm:");

            let layout_options = [
                (LayoutProvider::new_dot(), "Dot"),
                (LayoutProvider::new_force_directed(), "Force Directed"),
                (
                    LayoutProvider::new_fruchterman_reingold(),
                    "Fruchterman-Reingold",
                ),
            ];

            for (l, t) in layout_options {
                if ui.re_radio_value(&mut self.layout_provider, l, t).changed() {
                    self.layout = None
                };
            }
        });
        ui.end_row();
    }
}

impl SpaceViewState for GraphSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
