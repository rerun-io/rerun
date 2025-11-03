use egui::RichText;
use re_auth::Jwt;
use re_redap_client::ConnectionRegistryHandle;
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_uri::Scheme;
use re_viewer_context::{
    CommandSender, DisplayMode, GlobalContext, SystemCommand, SystemCommandSender as _,
};
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

#[derive(Default)]
struct Authentication {
    email: Option<String>,
    token: String,
    state: AuthenticationState,
}

#[derive(Default)]
enum AuthenticationState {
    #[default]
    LoginOrAddToken,
    TokenInput,
    LoginFlow(LoginFlow),
    Error(String),
}

impl Authentication {
    fn new(token: Option<String>) -> Self {
        let email = re_auth::oauth::load_credentials()
            .ok()
            .flatten()
            .map(|credentials| credentials.user().email.clone());
        let (token, state) = match token {
            Some(token) => (token, AuthenticationState::TokenInput),
            None => (String::new(), AuthenticationState::LoginOrAddToken),
        };

        Self {
            email,
            token,
            state,
        }
    }
}

pub struct ServerModal {
    modal: ModalHandler,

    mode: ServerModalMode,
    scheme: Scheme,
    host: String,
    auth: Authentication,
    port: u16,
}

impl Default for ServerModal {
    fn default() -> Self {
        Self {
            modal: Default::default(),
            mode: ServerModalMode::Add,
            scheme: Scheme::Rerun,
            host: String::new(),
            auth: Authentication::default(),
            port: 443,
        }
    }
}

impl ServerModal {
    pub fn open(&mut self, mode: ServerModalMode, connection_registry: &ConnectionRegistryHandle) {
        *self = match mode {
            ServerModalMode::Add => {
                let auth = Authentication::new(None);

                Self {
                    mode: ServerModalMode::Add,
                    auth,
                    ..Default::default()
                }
            }
            ServerModalMode::Edit(origin) => {
                let re_uri::Origin { scheme, host, port } = origin.clone();

                let credentials = connection_registry.credentials(&origin);
                let auth = match credentials {
                    Some(re_redap_client::Credentials::Token(token)) => {
                        Authentication::new(Some(token.to_string()))
                    }
                    Some(re_redap_client::Credentials::Stored) | None => Authentication::new(None),
                };

                Self {
                    modal: Default::default(),
                    mode: ServerModalMode::Edit(origin),
                    scheme,
                    host: host.to_string(),
                    auth,
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
                        if url::Host::parse(&self.host).is_err() {
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

                let mut host = url::Host::parse(&self.host);
                if host.is_err()
                    && let Ok(url) = url::Url::parse(&self.host)
                {
                    // Maybe the user pasted a full URL, with scheme and port?
                    // Then handle that gracefully! `from_str` requires the url
                    // with the "://" part so we just pass the whole url.
                    match url.scheme() {
                        "https" => self.scheme = Scheme::RerunHttps,
                        "http" => self.scheme = Scheme::RerunHttp,
                        _ => {
                            if let Ok(scheme) = Scheme::from_str(&self.host) {
                                self.scheme = scheme;
                            }
                        }
                    }

                    if let Some(url_host) = url.host_str() {
                        self.host = url_host.to_owned();
                        host = url::Host::parse(&self.host);
                    }

                    if let Some(port) = url.port() {
                        self.port = port;
                    }
                }

                ui.add_space(14.0);

                ui.label("Authenticate:");
                ui.scope(|ui| auth_ui(ui, global_ctx.command_sender, &mut self.auth));

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

                let credentials = if !self.auth.token.is_empty() {
                    Jwt::try_from(self.auth.token.clone())
                        .map(re_redap_client::Credentials::Token)
                        .map(Some)
                        // error is reported in the UI above
                        .map_err(|_err| ())
                } else if self.auth.email.is_some() {
                    Ok(Some(re_redap_client::Credentials::Stored))
                } else {
                    Ok(None)
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
                        self.auth.state = AuthenticationState::LoginOrAddToken;
                        ui.close();
                    }
                });
            },
        );
    }
}

fn auth_ui(ui: &mut egui::Ui, cmd: &CommandSender, auth: &mut Authentication) {
    ui.horizontal(|ui| {
        ui.scope(|ui| {
            let mut new_state = None;

            match &mut auth.state {
                AuthenticationState::LoginOrAddToken => {
                    if let Some(email) = &auth.email {
                        ui.label("Continue as ");
                        ui.label(RichText::new(email).strong().underline());

                        if ui
                            .small_icon_button(&re_ui::icons::CLOSE, "Clear login status")
                            .on_hover_text("Clear login status")
                            .clicked()
                        {
                            auth.email = None;
                        }
                    } else {
                        if action_button(ui, &mut true, None, "Login", "Login") {
                            new_state = match LoginFlow::open(ui) {
                                Ok(flow) => Some(AuthenticationState::LoginFlow(flow)),
                                Err(err) => Some(AuthenticationState::Error(err)),
                            };
                        }
                    }

                    ui.add_space(6.0);
                    ui.label("or");
                    ui.add_space(6.0);

                    if ui
                        .link(RichText::new("Add a token").strong().underline())
                        .clicked()
                    {
                        new_state = Some(AuthenticationState::TokenInput);
                    }
                }
                AuthenticationState::TokenInput => {
                    let jwt = (!auth.token.is_empty())
                        .then(|| re_auth::Jwt::try_from(auth.token.clone()))
                        .transpose();

                    if jwt.is_err() {
                        ui.style_invalid_field();
                    }

                    ui.add(
                        egui::TextEdit::singleline(&mut auth.token)
                            .hint_text("Token (will be stored in plain text)")
                            .code_editor(),
                    );

                    if ui
                        .small_icon_button(&re_ui::icons::CLOSE, "Go back")
                        .on_hover_text("Go back")
                        .clicked()
                    {
                        new_state = Some(AuthenticationState::LoginOrAddToken);
                    }
                }
                AuthenticationState::LoginFlow(flow) => {
                    if let Some(result) = flow.ui(ui, cmd) {
                        match result {
                            LoginFlowResult::Success(credentials) => {
                                auth.email = Some(credentials.user().email.clone());
                                new_state = Some(AuthenticationState::LoginOrAddToken);
                            }
                            LoginFlowResult::Failure(err) => {
                                new_state = Some(AuthenticationState::Error(err));
                            }
                        }
                    }
                }
                AuthenticationState::Error(err) => {
                    ui.error_label(err.clone());
                }
            }

            if let Some(new_state) = new_state {
                auth.state = new_state;
            }
        });
    });
}
