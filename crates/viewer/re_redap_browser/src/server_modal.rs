use egui::RichText;
use re_auth::Jwt;
use re_redap_client::ConnectionRegistryHandle;
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_uri::Scheme;
use re_viewer_context::{DisplayMode, GlobalContext, SystemCommand, SystemCommandSender as _};
use std::str::FromStr as _;

use crate::{context::Context, servers::Command};

mod login_flow;
use login_flow::{LoginFlow, LoginFlowResult, action_button};

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

enum CredentialsState {
    Token(String),
    Login(LoginFlow),
    LoggedIn(String),
    Error(String),
}

impl CredentialsState {
    #[expect(clippy::needless_pass_by_value)]
    fn with_token(token: String) -> Self {
        Self::Token(token.trim().to_owned())
    }

    fn empty_token() -> Self {
        Self::Token(String::new())
    }

    fn login_flow(ui: &mut egui::Ui) -> Self {
        match LoginFlow::open(ui) {
            Ok(flow) => Self::Login(flow),
            Err(err) => Self::Error(err),
        }
    }

    fn try_from_stored() -> Option<Self> {
        re_auth::oauth::load_credentials()
            .ok()
            .flatten()
            .map(|credentials| Self::LoggedIn(credentials.user().email.clone()))
    }
}

pub struct ServerModal {
    modal: ModalHandler,

    mode: ServerModalMode,
    scheme: Scheme,
    host: String,
    credentials: Option<CredentialsState>,
    port: u16,
}

impl Default for ServerModal {
    fn default() -> Self {
        Self {
            modal: Default::default(),
            mode: ServerModalMode::Add,
            scheme: Scheme::Rerun,
            host: String::new(),
            credentials: None,
            port: 443,
        }
    }
}

impl ServerModal {
    pub fn open(&mut self, mode: ServerModalMode, connection_registry: &ConnectionRegistryHandle) {
        *self = match mode {
            ServerModalMode::Add => {
                let credentials = CredentialsState::try_from_stored();

                Self {
                    mode: ServerModalMode::Add,
                    credentials,
                    ..Default::default()
                }
            }
            ServerModalMode::Edit(origin) => {
                let re_uri::Origin { scheme, host, port } = origin.clone();

                let credentials = connection_registry.credentials(&origin);
                let credentials = match credentials {
                    Some(re_redap_client::Credentials::Token(token)) => {
                        Some(CredentialsState::Token(token.to_string()))
                    }
                    Some(re_redap_client::Credentials::Stored) | None => {
                        CredentialsState::try_from_stored()
                    }
                };

                Self {
                    modal: Default::default(),
                    mode: ServerModalMode::Edit(origin),
                    scheme,
                    host: host.to_string(),
                    credentials,
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

                ui.label("URL:");
                let mut host = url::Host::parse(&self.host);

                if host.is_err()
                    && let Ok(url) = url::Url::parse(&self.host)
                {
                    // Maybe the user pasted a full URL, with scheme and port?
                    // Then handle that gracefully! `from_str` requires the url
                    // with the "://" part so we just pass the whole url.
                    if let Ok(scheme) = Scheme::from_str(&self.host) {
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

                ui.horizontal(|ui| {
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

                    ui.scope(|ui| {
                        // make field red if host is invalid
                        if host.is_err() {
                            ui.style_invalid_field();
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.host)
                                .lock_focus(false)
                                .hint_text("Host name")
                                .desired_width(200.0),
                        );
                        self.host = self.host.trim().to_owned();
                    });

                    ui.add(egui::DragValue::new(&mut self.port));
                });

                ui.add_space(14.0);

                ui.label("Authenticate:");
                ui.scope(|ui| match &mut self.credentials {
                    None => {
                        ui.horizontal(|ui| {
                            if action_button(ui, &mut true, None, "Login", "Login") {
                                self.credentials = Some(CredentialsState::login_flow(ui));
                            }

                            ui.add_space(6.0);
                            ui.label("or");
                            ui.add_space(6.0);

                            if ui
                                .link(RichText::new("Add a token").strong().underline())
                                .clicked()
                            {
                                self.credentials = Some(CredentialsState::empty_token());
                            }
                        });
                    }
                    Some(CredentialsState::Token(token)) => {
                        let mut token = token.clone();
                        let mut close = false;

                        ui.horizontal(|ui| {
                            let jwt = (!token.is_empty())
                                .then(|| re_auth::Jwt::try_from(token.clone()))
                                .transpose();

                            if jwt.is_err() {
                                ui.style_invalid_field();
                            }

                            ui.add(
                                egui::TextEdit::singleline(&mut token)
                                    .hint_text("Token (will be stored in plain text)")
                                    .code_editor(),
                            );

                            if ui
                                .small_icon_button(&re_ui::icons::CLOSE, "Clear login status")
                                .on_hover_text("Clear login status")
                                .clicked()
                            {
                                close = true;
                            }
                        });

                        if close {
                            self.credentials = None;
                        } else {
                            self.credentials = Some(CredentialsState::with_token(token));
                        }
                    }
                    Some(CredentialsState::Login(flow)) => {
                        if let Some(result) = flow.ui(ui, global_ctx.command_sender) {
                            match result {
                                LoginFlowResult::Success(credentials) => {
                                    self.credentials = Some(CredentialsState::LoggedIn(
                                        credentials.user().email.clone(),
                                    ));
                                }
                                LoginFlowResult::Failure(err) => {
                                    self.credentials = Some(CredentialsState::Error(err));
                                }
                            }
                        }
                    }
                    Some(CredentialsState::Error(err)) => {
                        ui.error_label(err.clone());
                    }
                    Some(CredentialsState::LoggedIn(email)) => {
                        let email = email.clone();
                        ui.scope(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Continue as ");
                                ui.label(RichText::new(email).strong().underline());

                                ui.add_space(6.0);
                                ui.label("or");
                                ui.add_space(6.0);

                                if ui.link(RichText::new("Add a token").underline()).clicked() {
                                    self.credentials = Some(CredentialsState::Token(String::new()));
                                }

                                if ui
                                    .small_icon_button(&re_ui::icons::CLOSE, "Clear login status")
                                    .on_hover_text("Clear login status")
                                    .clicked()
                                {
                                    self.credentials = None;
                                }
                            });
                        });
                    }
                });

                ui.add_space(24.0);

                let save_text = match &self.mode {
                    ServerModalMode::Add => "Add",
                    ServerModalMode::Edit(_) => "Save",
                };

                let origin = host.map(|host| re_uri::Origin {
                    scheme: self.scheme,
                    host,
                    port: self.port,
                });

                let credentials = match &self.credentials {
                    Some(CredentialsState::Token(token)) => Jwt::try_from(token.clone())
                        .map(re_redap_client::Credentials::Token)
                        .map(Some)
                        // error is reported in the UI above
                        .map_err(|_err| ()),
                    Some(CredentialsState::LoggedIn(_)) => {
                        Ok(Some(re_redap_client::Credentials::Stored))
                    }
                    Some(CredentialsState::Login(_) | CredentialsState::Error(_)) => Err(()),
                    None => Ok(None),
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button_width = ui.tokens().modal_button_width;

                    if let (Ok(origin), Ok(credentials)) = (origin, credentials) {
                        let save_button_response = ui.add(
                            egui::Button::new(save_text).min_size(egui::vec2(button_width, 0.0)),
                        );
                        if save_button_response.clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            ui.close();

                            if let ServerModalMode::Edit(old_origin) = &self.mode {
                                ctx.command_sender
                                    .send(Command::RemoveServer(old_origin.clone()))
                                    .ok();
                            }
                            ctx.command_sender
                                .send(Command::AddServer(origin.clone(), credentials))
                                .ok();
                            global_ctx.command_sender.send_system(
                                SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(origin)),
                            );
                        }
                    } else {
                        ui.add_enabled(false, egui::Button::new(save_text));
                    }

                    let cancel_button_response =
                        ui.add(egui::Button::new("Cancel").min_size(egui::vec2(button_width, 0.0)));
                    if cancel_button_response.clicked() {
                        ui.close();
                    }
                });
            },
        );
    }
}
