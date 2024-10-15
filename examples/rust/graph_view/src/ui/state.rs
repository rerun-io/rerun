use std::collections::HashMap;

use re_format::format_f32;
use re_viewer::external::{egui, re_ui::UiExt, re_viewer_context::SpaceViewState};

use crate::{graph::NodeIndex, layout::LayoutProvider};

use super::{bounding_rect_from_iter, scene::ViewBuilder};

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub(crate) struct GraphSpaceViewState {
    pub viewer: ViewBuilder,

    /// Indicates if the viewer should fit to the screen the next time it is rendered.
    pub should_fit_to_screen: bool,

    /// Positions of the nodes in world space.
    pub layout: Option<HashMap<NodeIndex, egui::Rect>>,
    pub layout_provider: LayoutProvider,
}

impl GraphSpaceViewState {
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
                self.should_fit_to_screen = true;
            }
        }
    }

    pub fn debug_ui(&mut self, ui: &mut egui::Ui) {
        ui.re_checkbox(&mut self.viewer.show_debug, "Show debug information")
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
