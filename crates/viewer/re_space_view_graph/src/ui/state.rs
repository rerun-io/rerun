use std::collections::HashMap;

use re_chunk::EntityPath;
use re_format::format_f32;
use re_ui::UiExt;
use re_viewer_context::SpaceViewState;

use crate::graph::NodeIndex;

use super::{bounding_rect_from_iter, scene::ViewBuilder};

#[derive(Debug)]
pub struct RadialLayoutConfig {
    pub circle_radius: f32,
}

impl Default for RadialLayoutConfig {
    fn default() -> Self {
        Self {
            circle_radius: 400.0,
        }
    }
}

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState {
    pub viewer: ViewBuilder,

    /// Indicates if the viewer should fit to the screen the next time it is rendered.
    pub should_fit_to_screen: bool,
    pub should_tick: bool,

    /// Positions of the nodes in world space.
    pub layout: HashMap<NodeIndex, egui::Rect>,

    /// Layout properties.
    pub layout_config: RadialLayoutConfig,
}

impl GraphSpaceViewState {
    pub fn bounding_box_ui(&mut self, ui: &mut egui::Ui) {
        ui.grid_left_hand_label("Bounding box")
            .on_hover_text("The bounding box encompassing all Entities in the view right now");
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if let Some(egui::Rect { min, max }) = bounding_rect_from_iter(self.layout.values()) {
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
            self.should_fit_to_screen = true;
        }
    }

    pub fn debug_ui(&mut self, ui: &mut egui::Ui) {
        ui.re_checkbox(&mut self.viewer.show_debug, "Show debug information")
            .on_hover_text("Shows debug information for the current graph");
        ui.end_row();
    }

    pub fn simulation_ui(&mut self, ui: &mut egui::Ui) {
        ui.grid_left_hand_label("Simulation")
            .on_hover_text("Control the simulation of the graph layout");
        if ui.button("Tick").clicked() {
            self.should_tick = true;
        }

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
