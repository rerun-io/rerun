use std::str::FromStr as _;

use re_grpc_client::ConnectionRegistryHandle;
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_uri::Scheme;
use re_viewer_context::{DisplayMode, GlobalContext, SystemCommand, SystemCommandSender as _};

use crate::{context::Context, servers::Command};

/// Should the modal edit an existing server or add a new one?
pub enum ServerModalMode {
    /// Show an empty modal to add a new server.
    Add,

    /// Show a modal to edit an existing server.
    ///
    /// You should ensure that the [`re_uri::Origin`] exists. (Otherwise, this leads to bad UX,
    /// since the modal will be titled "Edit server" but for the user it's a new server.)
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
                    .token(&origin)
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
    pub fn ui(&mut self, global_ctx: &GlobalContext<'_>, ctx: &Context<'_>, ui: &egui::Ui) {
        self.modal.ui(
            ui.ctx(),
            || {
                let title = match &self.mode {
                    ServerModalMode::Add => "Add server".to_owned(),
                    ServerModalMode::Edit(origin) => {
                        format!("Edit server: {}", origin.host)
                    }
                };
                ModalWrapper::new(&title)
            },
            |ui| {
                ui.warning_label(
                    "The dataplatform is very experimental and not generally \
                available yet. Proceed with caution!",
                );
                ui.label("Scheme:");

                egui::ComboBox::new("scheme", "")
                    .selected_text(if self.scheme == Scheme::RerunHttp {
                        "http"
                    } else {
                        "https"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.scheme, Scheme::RerunHttps, "https");
                        ui.selectable_value(&mut self.scheme, Scheme::RerunHttp, "http");
                    });

                ui.add_space(14.0);

                ui.label("Host name:");
                let mut host = url::Host::parse(&self.host);

                if host.is_err() {
                    if let Ok(url) = url::Url::parse(&self.host) {
                        // Maybe the user pasted a full URL, with scheme and port?
                        // Then handle that gracefully!
                        if let Ok(scheme) = Scheme::from_str(url.scheme()) {
                            self.scheme = scheme;
                        }

                        if let Some(url_host) = url.host_str() {
                            self.host = url_host.to_owned();
                            host = url::Host::parse(&self.host);
                        }

                        if let Some(port) = url.port() {
                            self.port = port;
                        }
                    }
                }

                ui.scope(|ui| {
                    // make field red if host is invalid
                    if host.is_err() {
                        style_invalid_field(ui);
                    }
                    ui.add(egui::TextEdit::singleline(&mut self.host).lock_focus(false));
                    self.host = self.host.trim().to_owned();
                });

                ui.add_space(14.0);

                ui.label("Token (optional, will be stored in clear text):");
                let token = ui
                    .scope(|ui| {
                        let token = (!self.token.is_empty())
                            .then(|| re_auth::Jwt::try_from(self.token.clone()))
                            .transpose();

                        if token.is_err() {
                            style_invalid_field(ui);
                        }

                        ui.add(egui::TextEdit::singleline(&mut self.token));
                        self.token = self.token.trim().to_owned();

                        token
                    })
                    .inner;

                ui.add_space(14.0);

                ui.label("Port:");
                ui.add(egui::DragValue::new(&mut self.port));

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
                        if ui.button(save_text).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            ui.close();

                            if let ServerModalMode::Edit(old_origin) = &self.mode {
                                ctx.command_sender
                                    .send(Command::RemoveServer(old_origin.clone()))
                                    .ok();
                            }
                            ctx.command_sender
                                .send(Command::AddServer(origin.clone(), token))
                                .ok();
                            global_ctx.command_sender.send_system(
                                SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(origin)),
                            );
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
