//! Implement a global drag-and-drop payload type that enable dragging from various parts of the UI
//! (e.g., from the streams tree to the viewport, etc.).

use std::fmt::Formatter;

use crate::{Contents, ItemCollection};

//TODO(ab): add more type of things we can drag, in particular entity paths
#[derive(Debug)]
pub enum DragAndDropPayload {
    /// The dragged content is made only of [`Contents`].
    Contents { contents: Vec<Contents> },

    /// The dragged content is made of a collection of [`Item`]s we do know how to handle.
    Invalid { items: crate::ItemCollection },
}

impl DragAndDropPayload {
    pub fn from_items(selected_items: ItemCollection) -> Self {
        if let Ok(contents) = (&selected_items).try_into() {
            Self::Contents { contents }
        } else {
            Self::Invalid {
                items: selected_items,
            }
        }
    }
}

impl std::fmt::Display for DragAndDropPayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contents { contents } => {
                let (container_cnt, view_cnt) = contents.iter().fold(
                    (0, 0),
                    |(container_cnt, view_cnt), content| match content {
                        Contents::Container(_) => (container_cnt + 1, view_cnt),
                        Contents::SpaceView(_) => (container_cnt, view_cnt + 1),
                    },
                );

                match (container_cnt, view_cnt) {
                    (0, 0) => write!(f, "empty"), // should not happen
                    (n, 0) => write!(f, "{n} container{}", if n > 1 { "s" } else { "" }),
                    (0, n) => write!(f, "{n} view{}", if n > 1 { "s" } else { "" }),
                    (container_cnt, view_cnt) => {
                        write!(
                            f,
                            "{container_cnt} container{}, {view_cnt} view{}",
                            if container_cnt > 1 { "s" } else { "" },
                            if view_cnt > 1 { "s" } else { "" }
                        )
                    }
                }
            }
            Self::Invalid { .. } => {
                //TODO
                write!(f, "invalid")
            }
        }
    }
}

/// Display the currently dragged payload as a pill in the UI.
///
/// This should be called once per frame.
pub fn drag_and_drop_payload_cursor_ui(ui: &mut egui::Ui) {
    if let Some(payload) = egui::DragAndDrop::payload::<DragAndDropPayload>(ui.ctx()) {
        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let layer_id = egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("drag_and_drop_payload_layer"),
            );
            let response = ui
                .scope_builder(egui::UiBuilder::new().layer_id(layer_id), |ui| {
                    //TODO: adjust look based on design
                    egui::Frame {
                        rounding: egui::Rounding::same(100.0), // max the rounding for a pill look
                        fill: ui.visuals().widgets.active.text_color(),
                        inner_margin: egui::Margin {
                            left: 8.0,
                            right: 8.0,
                            top: 4.0,
                            bottom: 3.0,
                        },
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(payload.to_string()).color(ui.visuals().panel_fill),
                        )
                    })
                    .inner
                })
                .response;

            //TODO: adjust geometry
            let delta = pointer_pos - response.rect.right_bottom();
            ui.ctx()
                .transform_layer_shapes(layer_id, emath::TSTransform::from_translation(delta));
        }
    }
}

//TODO: capture escape key to cancel drag
