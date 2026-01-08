use std::str::FromStr as _;

use egui::{Direction, Layout, OpenUrl, RichText};
use egui_extras::{Size, StripBuilder};
use re_auth::Jwt;
use re_redap_client::ConnectionRegistryHandle;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{ReButton, UiExt as _};
use re_uri::Scheme;
use re_viewer_context::{
    DisplayMode, EditRedapServerModalCommand, GlobalContext, SystemCommand,
    SystemCommandSender as _,
};

use crate::context::Context;
use crate::servers::Command;

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
    Edit(EditRedapServerModalCommand),
}

impl ServerModalMode {
    /// Should we show a warning about dataplatform being experimental?
    pub fn should_show_experimental_warning(&self) -> bool {
        matches!(self, Self::Add)
    }
}

#[expect(clippy::large_enum_variant)]
enum AuthKind {
    None,
    Token(String),
    RerunAccount(Option<LoginFlow>),
}

/// Authentication state for the server modal.
struct Authentication {
    kind: AuthKind,
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
    fn new(kind: AuthKind) -> Self {
        Self { kind, error: None }
    }

    /// This cleans up the login flow's resources, such as
    /// closing popup windows.
    fn reset_login_flow(&mut self) {
        if let AuthKind::RerunAccount(flow) = &mut self.kind {
            *flow = None;
        }
    }

    fn start_login_flow(&mut self, ui: &mut egui::Ui) {
        match LoginFlow::open(ui) {
            Ok(flow) => {
                self.kind = AuthKind::RerunAccount(Some(flow));
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
            auth: Authentication::new(AuthKind::RerunAccount(None)),
            port: 443,
        }
    }
}

impl ServerModal {
    pub fn open(&mut self, mode: ServerModalMode, connection_registry: &ConnectionRegistryHandle) {
        *self = match mode {
            ServerModalMode::Add => {
                let auth = Authentication::new(AuthKind::RerunAccount(None));

                Self {
                    mode: ServerModalMode::Add,
                    auth,
                    ..Default::default()
                }
            }
            ServerModalMode::Edit(edit) => {
                let re_uri::Origin { scheme, host, port } = edit.origin.clone();

                let credentials = connection_registry.credentials(&edit.origin);
                let auth = match credentials {
                    Some(re_redap_client::Credentials::Token(token)) => {
                        Authentication::new(AuthKind::Token(token.to_string()))
                    }
                    Some(re_redap_client::Credentials::Stored) => {
                        Authentication::new(AuthKind::RerunAccount(None))
                    }
                    None => Authentication::new(AuthKind::None),
                };

                Self {
                    modal: Default::default(),
                    mode: ServerModalMode::Edit(edit),
                    scheme,
                    host: host.to_string(),
                    auth,
                    port,
                }
            }
        };

        self.modal.open();
    }

    pub fn logout(&mut self) {
        self.auth.reset_login_flow();
    }

    pub fn ui(&mut self, global_ctx: &GlobalContext<'_>, ctx: &Context<'_>, ui: &egui::Ui) {
        let was_open = self.modal.is_open();

        self.modal.ui(
            ui.ctx(),
            || {
                let title = match &self.mode {
                    ServerModalMode::Add => "Add server".to_owned(),
                    ServerModalMode::Edit(edit) => {
                        if let Some(title) = &edit.title {
                            title.clone()
                        } else {
                            format!("Edit server: {}", edit.origin.host)
                        }
                    }
                };
                ModalWrapper::new(&title)
                    .default_height(300.0)
                    .min_height(300.0)
            },
            |ui| {
                if self.mode.should_show_experimental_warning() {
                    ui.warning_label(
                        "The dataplatform is very experimental and not generally \
                available yet. Proceed with caution!",
                    );
                }

                let label = ui.label("Address:");

                egui::Sides::new()
                    .shrink_left()
                    .height(ui.spacing().interact_size.y)
                    .show(
                        ui,
                        |ui| {
                            egui::ComboBox::new("scheme", "")
                                .selected_text(if self.scheme == Scheme::RerunHttp {
                                    "http"
                                } else {
                                    "https"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.scheme,
                                        Scheme::RerunHttps,
                                        "https",
                                    );
                                    ui.selectable_value(
                                        &mut self.scheme,
                                        Scheme::RerunHttp,
                                        "http",
                                    );
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
                                        .desired_width(ui.available_width()),
                                )
                                .labelled_by(label.id);
                                self.host = self.host.trim().to_owned();
                            });
                        },
                        |ui| {
                            ui.add(egui::DragValue::new(&mut self.port));
                        },
                    );

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

                ui.selectable_toggle(|ui| {
                    StripBuilder::new(ui)
                        .sizes(Size::relative(1.0 / 3.0), 3)
                        .cell_layout(Layout::centered_and_justified(Direction::TopDown))
                        .horizontal(|mut strip| {
                            strip.cell(|ui| {
                                if ui
                                    .selectable_label(
                                        matches!(self.auth.kind, AuthKind::RerunAccount(_)),
                                        "Rerun account",
                                    )
                                    .clicked()
                                {
                                    self.auth.kind = AuthKind::RerunAccount(None);
                                }
                            });

                            strip.cell(|ui| {
                                if ui
                                    .selectable_label(
                                        matches!(self.auth.kind, AuthKind::Token(_)),
                                        "With a token",
                                    )
                                    .clicked()
                                {
                                    self.auth.kind = AuthKind::Token(String::new());
                                }
                            });

                            strip.cell(|ui| {
                                if ui
                                    .selectable_label(
                                        matches!(self.auth.kind, AuthKind::None),
                                        "No authentication",
                                    )
                                    .clicked()
                                {
                                    self.auth.kind = AuthKind::None;
                                }
                            });
                        });
                });

                auth_ui(ui, global_ctx, &mut self.auth);

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

                let credentials = match &self.auth.kind {
                    AuthKind::Token(token) => Jwt::try_from(token.clone())
                        .map(re_redap_client::Credentials::Token)
                        .map(Some)
                        .map_err(|_err| ()),
                    AuthKind::RerunAccount(_) => {
                        if global_ctx.logged_in() {
                            Ok(Some(re_redap_client::Credentials::Stored))
                        } else {
                            Err(())
                        }
                    }
                    AuthKind::None => Ok(None),
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                    let enabled = origin.is_ok() && credentials.is_ok();
                    let save_button_response =
                        ui.add_enabled(enabled, ReButton::new(save_text).primary().small());

                    if let Ok(origin) = origin
                        && let Ok(credentials) = credentials
                        && (save_button_response.clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        self.auth.reset_login_flow();
                        ui.close();

                        if let ServerModalMode::Edit(edit) = &self.mode {
                            ctx.command_sender
                                .send(Command::RemoveServer(edit.origin.clone()))
                                .ok();
                        }

                        let on_add: Box<dyn FnOnce() + Send> =
                            if let ServerModalMode::Edit(EditRedapServerModalCommand {
                                open_on_success: Some(url),
                                ..
                            }) = &self.mode
                            {
                                let egui_ctx = ui.ctx().clone();
                                let url = url.clone();
                                Box::new(move || {
                                    egui_ctx.open_url(OpenUrl::same_tab(url));
                                })
                            } else {
                                let command_sender = global_ctx.command_sender.clone();
                                let origin = origin.clone();
                                Box::new(move || {
                                    command_sender.send_system(SystemCommand::ChangeDisplayMode(
                                        DisplayMode::RedapServer(origin),
                                    ));
                                })
                            };

                        ctx.command_sender
                            .send(Command::AddServer {
                                origin: origin.clone(),
                                credentials,
                                on_add: Some(on_add),
                            })
                            .ok();
                    }

                    let cancel_button_response = ui.add(ReButton::new("Cancel").small());
                    if cancel_button_response.clicked() {
                        self.auth = Authentication::new(AuthKind::RerunAccount(None));
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

fn auth_ui(ui: &mut egui::Ui, ctx: &GlobalContext<'_>, auth: &mut Authentication) {
    match &mut auth.kind {
        AuthKind::RerunAccount(login_flow) => {
            ui.label("Rerun account:");

            if let Some(flow) = login_flow {
                // Login flow is in progress - show login buttons or spinner
                if let Some(result) = flow.ui(ui, ctx.command_sender) {
                    match result {
                        LoginFlowResult::Success => {
                            auth.error = None;
                            auth.reset_login_flow();
                        }
                        LoginFlowResult::Failure(err) => {
                            auth.error = Some(err);
                            auth.reset_login_flow();
                        }
                    }
                }
            } else if let Some(logged_in) = &ctx.auth_context {
                // User is logged in
                ui.horizontal(|ui| {
                    ui.label("Continue as");
                    ui.label(RichText::new(&logged_in.email).strong());
                });
            } else {
                // User is not logged in - start the login flow to show buttons
                auth.start_login_flow(ui);
            }

            if let Some(error) = &auth.error {
                ui.error_label(error.clone());
            }
        }

        AuthKind::Token(token) => {
            ui.label("Access token (will be stored in plain text):");

            ui.scope(|ui| {
                let jwt = (!token.is_empty())
                    .then(|| Jwt::try_from(token.clone()))
                    .transpose();

                if jwt.is_err() {
                    ui.style_invalid_field();
                }

                ui.add(
                    egui::TextEdit::singleline(token)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
        }

        AuthKind::None => {
            // No UI needed for "No authentication"
        }
    }
}
