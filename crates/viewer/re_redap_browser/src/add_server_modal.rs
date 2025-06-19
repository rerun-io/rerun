use egui::Ui;
use re_grpc_client::{ConnectionRegistry, ConnectionRegistryHandle};
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_uri::Scheme;

use crate::context::Context;
use crate::form::{Form, FormField};
use crate::servers::Command;

pub enum ServerModalMode {
    Add,
    Edit(re_uri::Origin),
}

pub struct ServerModal {
    modal: ModalHandler,

    mode: ServerModalMode,
    scheme: Scheme,
    host: String,
    token: String,
    port: u16,
}

impl Default for ServerModal {
    fn default() -> Self {
        Self {
            modal: Default::default(),
            mode: ServerModalMode::Add,
            scheme: Scheme::Rerun,
            host: String::new(),
            token: String::new(),
            port: 443,
        }
    }
}

impl ServerModal {
    pub fn open(&mut self, mode: ServerModalMode, connection_registry: &ConnectionRegistryHandle) {
        *self = match mode {
            ServerModalMode::Add => Default::default(),
            ServerModalMode::Edit(origin) => {
                let token = connection_registry
                    .get_token(&origin)
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                let re_uri::Origin { scheme, host, port } = origin.clone();

                Self {
                    modal: Default::default(),
                    mode: ServerModalMode::Edit(origin),
                    scheme,
                    host: host.to_string(),
                    token,
                    port,
                }
            }
        };

        self.modal.open();
    }

    //TODO(ab): handle ESC and return
    pub fn ui(
        &mut self,
        ctx: &Context<'_>,
        connection_registry: &ConnectionRegistryHandle,
        ui: &egui::Ui,
    ) {
        let title = match &self.mode {
            ServerModalMode::Add => "Add Server".to_owned(),
            ServerModalMode::Edit(origin) => {
                format!("Edit Server: {}", origin.host.to_string())
            }
        };
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new(&title).default_width(423.0),
            |ui| {
                ui.warning_label(
                    "The dataplatform is very experimental and not generally \
                available yet. Proceed with caution!",
                );

                let host = url::Host::parse(&self.host);
                let token = (!self.token.is_empty())
                    .then(|| re_auth::Jwt::try_from(self.token.clone()))
                    .transpose();

                Form::new(title.clone()).show(ui, |ui| {
                    FormField::new("Scheme").show(ui, |ui: &mut Ui| {
                        egui::ComboBox::new("scheme", "")
                            .selected_text(if self.scheme == Scheme::RerunHttp {
                                "http"
                            } else {
                                "https"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.scheme, Scheme::RerunHttps, "https");
                                ui.selectable_value(&mut self.scheme, Scheme::RerunHttp, "http");
                            })
                            .response
                    });

                    FormField::new("Host name")
                        .error(!self.host.is_empty() && host.is_err())
                        .show(ui, egui::TextEdit::singleline(&mut self.host));

                    FormField::new("Token")
                        .hint("Optional, will be stored in clear text")
                        .error(!self.token.is_empty() && token.is_err())
                        .show(ui, egui::TextEdit::singleline(&mut self.token));

                    FormField::new("Port")
                        .show(ui, egui::DragValue::new(&mut self.port).range(1..=65535));
                });

                let origin = host.map(|host| re_uri::Origin {
                    scheme: self.scheme,
                    host,
                    port: self.port,
                });

                ui.add_space(24.0);

                let save_text = match &self.mode {
                    ServerModalMode::Add => "Add",
                    ServerModalMode::Edit(_) => "Save",
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let (Ok(origin), Ok(token)) = (origin, token) {
                        if ui.button(save_text).clicked() {
                            ui.close();

                            if let Some(token) = token {
                                connection_registry.set_token(&origin, token);
                            }

                            match &self.mode {
                                ServerModalMode::Add => {
                                    ctx.command_sender.send(Command::AddServer(origin)).ok();
                                }
                                ServerModalMode::Edit(old_origin) => {
                                    ctx.command_sender
                                        .send(Command::UpdateServer {
                                            previous_origin: old_origin.clone(),
                                            new_origin: origin,
                                        })
                                        .ok();
                                }
                            }
                        }
                    } else {
                        ui.add_enabled(false, egui::Button::new(save_text));
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                });
            },
        );
    }
}

fn style_invalid_field(ui: &mut egui::Ui) {
    ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    ui.visuals_mut().widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    ui.visuals_mut().widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, ui.visuals().error_fg_color);
}
