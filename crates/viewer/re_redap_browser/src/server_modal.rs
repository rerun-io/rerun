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
use login_flow::{LoginFlow, LoginFlowResult};

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

/// Authentication state for the server modal.
struct Authentication {
    token: String,
    show_token_input: bool,
    login_flow: Option<LoginFlow>,
    error: Option<String>,
}

impl Authentication {
    /// Initialize auth state.
    ///
    /// This attempts to load credentials from disk if `use_stored_credentials`
    /// is set to `true`. Note that they are accepted even if they are expired,
    /// the assumption being that they'll be refreshed automatically before usage.
    ///
    /// Optionally, this can be given a token, which takes
    /// precedence over stored credentials.
    fn new(token: Option<String>, use_stored_credentials: bool) -> Self {
        let (token, show_token_input) = match token {
            Some(token) => (token, true),
            None => (String::new(), false),
        };

        Self {
            token,
            show_token_input,
            login_flow: None,
            error: None,
        }
    }

    /// This cleans up the login flow's resources, such as
    /// closing popup windows.
    fn reset_login_flow(&mut self) {
        self.login_flow = None;
    }

    fn start_login_flow(&mut self, ui: &mut egui::Ui) {
        // TODO: Is login hint required?
        match LoginFlow::open(ui, None) {
            Ok(flow) => {
                self.login_flow = Some(flow);
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err);
            }
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
            auth: Authentication::new(None, false),
            port: 443,
        }
    }
}

impl ServerModal {
    pub fn open(&mut self, mode: ServerModalMode, connection_registry: &ConnectionRegistryHandle) {
        let use_stored_credentials = connection_registry.should_use_stored_credentials();
        *self = match mode {
            ServerModalMode::Add => {
                let auth = Authentication::new(None, use_stored_credentials);

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
                        Authentication::new(Some(token.to_string()), use_stored_credentials)
                    }
                    Some(re_redap_client::Credentials::Stored) | None => {
                        Authentication::new(None, use_stored_credentials)
                    }
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

    pub fn ui(&mut self, global_ctx: &GlobalContext<'_>, ctx: &Context<'_>, ui: &egui::Ui) {
        let was_open = self.modal.is_open();

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
                    .default_height(300.0)
                    .min_height(300.0)
            },
            |ui| {
                ui.warning_label(
                    "The dataplatform is very experimental and not generally \
                available yet. Proceed with caution!",
                );

                let label = ui.label("URL:");

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
                        )
                        .labelled_by(label.id);
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
                ui.scope(|ui| {
                    ui.shrink_width_to_current();
                    auth_ui(ui, global_ctx, &mut self.auth);
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

                let credentials = if !self.auth.token.is_empty() {
                    Jwt::try_from(self.auth.token.clone())
                        .map(re_redap_client::Credentials::Token)
                        .map(Some)
                        // error is reported in the UI above
                        .map_err(|_err| ())
                } else if global_ctx.logged_in() {
                    Ok(Some(re_redap_client::Credentials::Stored))
                } else {
                    Ok(None)
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                    let button_width = ui.tokens().modal_button_width;

                    if let (Ok(origin), Ok(credentials)) = (origin, credentials) {
                        let save_button_response = ui.add(
                            egui::Button::new(save_text).min_size(egui::vec2(button_width, 0.0)),
                        );
                        if save_button_response.clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            self.auth.reset_login_flow();
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
                        self.auth.show_token_input = false;
                        self.auth.reset_login_flow();
                        ui.close();
                    }
                });
            },
        );

        // reset login flow if modal was just closed (e.g., by backdrop click)
        if was_open && !self.modal.is_open() {
            re_log::debug!("modal closed; reset login flow");
            self.auth.reset_login_flow();
        }
    }
}

fn auth_ui(ui: &mut egui::Ui, ctx: &GlobalContext, auth: &mut Authentication) {
    ui.horizontal(|ui| {
        ui.scope(|ui| {
            if auth.show_token_input {
                let jwt = (!auth.token.is_empty())
                    .then(|| re_auth::Jwt::try_from(auth.token.clone()))
                    .transpose();

                if jwt.is_err() {
                    ui.style_invalid_field();
                }

                ui.horizontal(|ui| {
                    ui.set_min_width(300.0);
                    ui.set_width(300.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut auth.token)
                            .hint_text("Token (will be stored in plain text)")
                            .code_editor()
                            .desired_width(300.0),
                    );
                });

                if ui
                    .small_icon_button(&re_ui::icons::CLOSE, "Go back")
                    .on_hover_text("Go back")
                    .clicked()
                {
                    auth.show_token_input = false;
                    auth.error = None;
                }
            } else {
                if let Some(flow) = &mut auth.login_flow {
                    if let Some(result) = flow.ui(ui, ctx.command_sender) {
                        match result {
                            LoginFlowResult::Success(credentials) => {
                                auth.error = None;
                                // Clear login flow to close popup window
                                auth.reset_login_flow();
                            }
                            LoginFlowResult::Failure(err) => {
                                auth.error = Some(err);
                                // Clear login flow so user can retry
                                auth.reset_login_flow();
                            }
                        }
                    }
                } else if let Some(logged_in) = &ctx.auth_context {
                    ui.label("Continue as ");
                    ui.label(RichText::new(&logged_in.email).strong().underline());

                    if ui
                        .small_icon_button(&re_ui::icons::CLOSE, "Clear login status")
                        .on_hover_text("Clear login status")
                        .clicked()
                    {
                        auth.error = None;
                        auth.start_login_flow(ui);
                    }
                } else if auth.error.is_some() {
                    if ui
                        .link(RichText::new("Login again").strong().underline())
                        .clicked()
                    {
                        auth.error = None;
                    }
                } else {
                    auth.start_login_flow(ui);
                }

                ui.add_space(6.0);
                ui.label("or");
                ui.add_space(6.0);

                if ui
                    .link(RichText::new("Add a token").strong().underline())
                    .clicked()
                {
                    auth.show_token_input = true;
                    auth.error = None;
                }
            }
        });
    });

    ui.horizontal(|ui| {
        ui.set_min_width(300.0);
        ui.set_width(300.0);
        if !auth.show_token_input && !ctx.logged_in() {
            if let Some(error) = &auth.error {
                ui.error_label(error.clone());
            }
        }
    });
}
