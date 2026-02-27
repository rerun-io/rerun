use std::time::Instant;

use rerun::external::egui::{self, ScrollArea};
use rerun::external::re_log::ResultExt;
use rerun::external::re_ui::{UiExt, list_item};
use rerun::external::{eframe, re_viewer};

use crate::comms::protocol::Message;
use crate::comms::viewer::ControlViewerHandle;

#[derive(Default)]
pub struct ControlStates {
    pub last_resource_update: Option<Instant>,
    pub controls_view: ControlsView,
    pub message_kind: ObjectKind,
    pub entity_path: String,
    pub position: (f32, f32, f32),
    pub half_size: (f32, f32, f32),
    pub radius: f32,
    pub dynamic_offset: f32,
    pub dynamic_radius: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ObjectKind {
    #[default]
    Point3d,
    Box3d,
}

#[derive(Default)]
pub struct ControlsView {
    pub key_sequence: Vec<String>,
}

pub struct Control {
    app: re_viewer::App,
    states: ControlStates,
    handle: ControlViewerHandle,
}

impl eframe::App for Control {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Store viewer state on disk
        self.app.save(storage);
    }

    /// Called whenever we need repainting, which could be 60 Hz.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // First add our panel(s):
        egui::Panel::right("Control Panel")
            .default_size(400.0)
            .show_inside(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    self.ui(ui);
                });
            });

        self.app.ui(ui, frame);
    }
}

impl Control {
    pub fn new(app: re_viewer::App, handle: ControlViewerHandle) -> Self {
        Control {
            app,
            states: ControlStates {
                entity_path: "foo".to_string(),
                dynamic_radius: 0.1,
                ..Default::default()
            },
            handle,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 9.0;

        ui.vertical_centered(|ui| {
            ui.strong("Control panel");
        });

        list_item::list_item_scope(ui, "Message properties", |ui| {
            ui.spacing_mut().item_spacing.y = ui.ctx().global_style().spacing.item_spacing.y;
            ui.section_collapsing_header("Message properties")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Message kind:");
                        ui.drop_down_menu(
                            "kind",
                            format!("{:?}", self.states.message_kind),
                            |ui| {
                                ui.selectable_value(
                                    &mut self.states.message_kind,
                                    ObjectKind::Point3d,
                                    "Point3d",
                                );
                                ui.selectable_value(
                                    &mut self.states.message_kind,
                                    ObjectKind::Box3d,
                                    "Box3d",
                                );
                            },
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Entity path:");
                        ui.text_edit_singleline(&mut self.states.entity_path);
                    });

                    match self.states.message_kind {
                        ObjectKind::Point3d => point_message_ui(ui, &mut self.states),
                        ObjectKind::Box3d => box_message_ui(ui, &mut self.states),
                    }
                });

            ui.horizontal(|ui| {
                if ui.button("Send message").clicked() {
                    match self.states.message_kind {
                        ObjectKind::Point3d => {
                            self.handle
                                .send(Message::Point3d {
                                    path: self.states.entity_path.clone(),
                                    position: self.states.position,
                                    radius: self.states.radius,
                                })
                                .expect("e");
                        }
                        ObjectKind::Box3d => {
                            self.handle
                                .send(Message::Box3d {
                                    path: self.states.entity_path.clone(),
                                    position: self.states.position,
                                    half_size: self.states.half_size,
                                })
                                .expect("b");
                        }
                    }
                }
            });
        });

        list_item::list_item_scope(ui, "dynamic", |ui| {
            ui.spacing_mut().item_spacing.y = ui.ctx().global_style().spacing.item_spacing.y;
            ui.section_collapsing_header("Dynamic position")
                .default_open(true)
                .show(ui, |ui| {
                    dynamic_control_ui(ui, self.handle.clone(), &mut self.states);
                });
        });
    }
}

fn point_message_ui(ui: &mut egui::Ui, states: &mut ControlStates) {
    position_ui(states, ui);

    ui.horizontal(|ui| {
        ui.label("Radius:");
        ui.add(egui::widgets::DragValue::new(&mut states.radius).speed(0.1));
    });
}

fn box_message_ui(ui: &mut egui::Ui, states: &mut ControlStates) {
    position_ui(states, ui);

    let mut half_size = states.half_size;
    ui.horizontal(|ui| {
        ui.label("Half size:");
        ui.add(egui::widgets::DragValue::new(&mut half_size.0).speed(0.1));
        ui.add(egui::widgets::DragValue::new(&mut half_size.1).speed(0.1));
        ui.add(egui::widgets::DragValue::new(&mut half_size.2).speed(0.1));
    });

    states.half_size = half_size;
}

fn position_ui(states: &mut ControlStates, ui: &mut egui::Ui) {
    let mut position = states.position;

    ui.horizontal(|ui| {
        ui.label("Position:");
        ui.add(egui::widgets::DragValue::new(&mut position.0).speed(0.1));
        ui.add(egui::widgets::DragValue::new(&mut position.1).speed(0.1));
        ui.add(egui::widgets::DragValue::new(&mut position.2).speed(0.1));
    });

    states.position = position;
}

fn dynamic_control_ui(ui: &mut egui::Ui, handle: ControlViewerHandle, states: &mut ControlStates) {
    ui.horizontal(|ui| {
        ui.label("Offset:");
        if ui
            .add(egui::Slider::new(
                &mut states.dynamic_offset,
                0.1_f32..=10_f32,
            ))
            .changed()
        {
            handle
                .send(Message::DynamicPosition {
                    radius: states.dynamic_radius,
                    offset: states.dynamic_offset,
                })
                .warn_on_err_once("Failed to send message");
        }
    });

    ui.horizontal(|ui| {
        ui.label("Radius:");
        if ui
            .add(egui::Slider::new(
                &mut states.dynamic_radius,
                0.1_f32..=1_f32,
            ))
            .changed()
        {
            handle
                .send(Message::DynamicPosition {
                    radius: states.dynamic_radius,
                    offset: states.dynamic_offset,
                })
                .warn_on_err_once("Failed to send message");
        }
    });
}
