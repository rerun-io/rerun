use ahash::HashMap;
use egui::Rect;
use re_chunk::{EntityPath, TimeInt, Timeline};
use re_format::format_f32;
use re_types::blueprint::components::VisualBounds2D;
use re_ui::UiExt;
use re_viewer_context::SpaceViewState;

use crate::layout::Layout;

use super::bounding_rect_from_iter;

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState {
    pub layout: LayoutState,

    pub show_debug: bool,

    pub world_bounds: Option<VisualBounds2D>,
}

impl GraphSpaceViewState {
    pub fn layout_ui(&mut self, ui: &mut egui::Ui) {
        let Some(rect) = self.layout.bounding_rect() else {
            return;
        };
        ui.grid_left_hand_label("Layout")
            .on_hover_text("The bounding box encompassing all entities in the view right now");
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let egui::Rect { min, max } = rect;
            ui.label(format!("x [{} - {}]", format_f32(min.x), format_f32(max.x),));
            ui.label(format!("y [{} - {}]", format_f32(min.y), format_f32(max.y),));
        });
        ui.end_row();
    }

    pub fn debug_ui(&mut self, ui: &mut egui::Ui) {
        ui.re_checkbox(&mut self.show_debug, "Show debug information")
            .on_hover_text("Shows debug information for the current graph");
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

/// Used to determine if a layout is up-to-date or outdated.
#[derive(Debug, PartialEq, Eq)]
pub struct Timestamp {
    timeline: Timeline,
    time: TimeInt,
}

/// The following is a simple state machine that keeps track of the different
/// layouts and if they need to be recomputed. It also holds the state of the
/// force-based simulation.
#[derive(Default)]
pub enum LayoutState {
    #[default]
    None,
    Outdated {
        timestamp: Timestamp,
        layouts: HashMap<EntityPath, Layout>,
    },
    Finished {
        timestamp: Timestamp,
        layouts: HashMap<EntityPath, Layout>,
    },
}

impl LayoutState {
    pub fn bounding_rect(&self) -> Option<Rect> {
        match self {
            Self::Outdated { layouts, .. } | Self::Finished { layouts, .. } => {
                let union_rect =
                    bounding_rect_from_iter(layouts.values().map(|l| l.bounding_rect()));
                Some(union_rect)
            }
            Self::None => None,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
