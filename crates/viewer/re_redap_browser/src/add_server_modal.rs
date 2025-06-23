use crate::context::Context;
use crate::servers::Command;
use re_grpc_client::ConnectionRegistryHandle;
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_uri::Scheme;
use re_viewer_context::{DisplayMode, GlobalContext, SystemCommand, SystemCommandSender as _};

pub struct AddServerModal {
    modal: ModalHandler,

    scheme: Scheme,
    host: String,
    token: String,
    port: u16,
}

impl Default for AddServerModal {
    fn default() -> Self {
        Self {
            modal: Default::default(),
            scheme: Scheme::Rerun,
            host: String::new(),
            token: String::new(),
            port: 443,
        }
    }
}

impl AddServerModal {
    pub fn open(&mut self) {
        self.scheme = Scheme::Rerun;
        self.port = 443;
        self.host = String::new();

        self.modal.open();
    }

    //TODO(ab): handle ESC and return
    pub fn ui(
        &mut self,
        global_ctx: &GlobalContext<'_>,
        ctx: &Context<'_>,
        connection_registry: &ConnectionRegistryHandle,
        ui: &egui::Ui,
    ) {
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Add Server"),
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
                let host = url::Host::parse(&self.host);
                ui.scope(|ui| {
                    // make field red if host is invalid
                    if host.is_err() {
                        style_invalid_field(ui);
                    }
                    ui.add(egui::TextEdit::singleline(&mut self.host).lock_focus(false));
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let (Ok(origin), Ok(token)) = (origin, token) {
                        if ui.button("Add").clicked() {
                            ui.close();

                            if let Some(token) = token {
                                connection_registry.set_token(&origin, token);
                            }

                            ctx.command_sender
                                .send(Command::AddServer(origin.clone()))
                                .ok();
                            global_ctx.command_sender.send_system(
                                SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(origin)),
                            );
                        }
                    } else {
                        ui.add_enabled(false, egui::Button::new("Add"));
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
