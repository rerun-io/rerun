use egui::Rect;
use re_chunk::{TimeInt, Timeline};
use re_format::format_f32;
use re_log::external::log;
use re_types::blueprint::components::VisualBounds2D;
use re_ui::UiExt;
use re_viewer_context::SpaceViewState;

use crate::{
    graph::Graph,
    layout::{ForceLayout, Layout},
};

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

impl Timestamp {
    pub fn new(timeline: Timeline, time: TimeInt) -> Self {
        Self { timeline, time }
    }
}

/// The following is a simple state machine that keeps track of the different
/// layouts and if they need to be recomputed. It also holds the state of the
/// force-based simulation.
#[derive(Default)]
pub enum LayoutState {
    #[default]
    None,
    InProgress {
        timestamp: Timestamp,
        layout: Layout,
        provider: ForceLayout,
    },
    Finished {
        timestamp: Timestamp,
        layout: Layout,
    },
}

impl LayoutState {
    pub fn bounding_rect(&self) -> Option<Rect> {
        match self {
            Self::Finished { layout, .. } => Some(layout.bounding_rect()),
            Self::None => None,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// This method is lazy. A new layout is only computed if the current timestamp requires it.
    pub fn update<'a>(
        &'a mut self,
        timeline: Timeline,
        time: TimeInt,
        graphs: impl Iterator<Item = &'a Graph<'a>> + Clone,
    ) -> &'a mut Layout {
        let requested = Timestamp::new(timeline, time);

        match self {
            Self::Finished { timestamp, .. } if timestamp == &requested => {
                return match self {
                    Self::Finished { layout, .. } => layout,
                    _ => unreachable!(), // We just checked that the state is `Self::Current`.
                };
            },
            Self::Finished { .. } => (), // TODO(grtlr): repurpose old layout
        }

        *self = Self::Finished {
            timestamp: requested,
            layout,
        };

        match self {
            Self::Finished { layout, .. } => layout,
            _ => unreachable!(), // We just set the state to `Self::Current` above.
        }
    }
}
