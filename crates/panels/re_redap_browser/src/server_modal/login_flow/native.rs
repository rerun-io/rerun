use std::sync::Arc;
use std::time::Duration;

use egui::mutex::Mutex;
use re_auth::callback_server::OauthCallbackServer;
use re_auth::oauth::Credentials;
use re_auth::oauth::api::{AuthenticateWithCode, Pkce, send_native};
use re_ui::{ContextExt as _, UiExt as _, Variant, icons};

use super::{ActionButton, LoginError};

pub struct State {
    callback_server: OauthCallbackServer,

    pkce: Pkce,
    pending_authentication: bool,
    credentials: Arc<Mutex<Option<Result<Credentials, LoginError>>>>,

    show_open_feedback: bool,
    show_copy_feedback: bool,
}

impl State {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // The native login flow uses oauth 2.0 code authorization flow with PKCE

        if self.pending_authentication {
            ui.loading_indicator("Waiting for native login");
        } else {
            ui.horizontal(|ui| {
                if ActionButton::new(&icons::EXTERNAL_LINK, "Log in", "Link opened!")
                    .variant(Variant::Primary)
                    .show(ui, &mut self.show_open_feedback)
                    .clicked()
                {
                    webbrowser::open(self.callback_server.get_login_url()).ok();
                }

                // Hide the button during tests, so that the native and wasm builds match visually.
                if !ui.ctx().is_test()
                    && ActionButton::new(&icons::COPY, "Copy link", "Copied to clipboard!")
                        .show(ui, &mut self.show_copy_feedback)
                        .clicked()
                {
                    ui.copy_text(self.callback_server.get_login_url().to_owned());
                }
            });
        }

        // Poll `Self::done` for the browser's redirect back to our callback server.
        //
        // Keep this comfortably above one frame time: `request_repaint_after` subtracts the
        // predicted frame time from the delay, so anything shorter saturates to zero and turns
        // this into a repaint on every single frame.
        ui.request_repaint_after(Duration::from_millis(300));
    }

    pub fn done(&mut self) -> Result<Option<Credentials>, LoginError> {
        if self.pending_authentication {
            if let Some(credentials) = (*self.credentials.lock()).take() {
                self.pending_authentication = false;
                return credentials.map(Some);
            }
            // don't check the callback server anymore
            return Ok(None);
        }

        // We're done if we received valid credentials from the client:
        match self.callback_server.check_for_browser_response() {
            Ok(Some(code)) => {
                let credentials = self.credentials.clone();
                let on_done = move |res: Result<Credentials, LoginError>| {
                    *credentials.lock() = Some(res);
                };
                send_native(
                    AuthenticateWithCode::new(&code, &self.pkce),
                    move |res| match res {
                        Ok(res) => on_done(
                            Credentials::from_auth_response(res.into())
                                .map_err(|source| LoginError::DecodeToken { source })
                                .and_then(|credentials| {
                                    credentials
                                        .ensure_stored()
                                        .map_err(|source| LoginError::StoreCredentials { source })
                                }),
                        ),
                        Err(source) => on_done(Err(LoginError::Authenticate { source })),
                    },
                );
                self.pending_authentication = true;

                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(source) => Err(LoginError::CheckBrowserResponse { source }),
        }
    }

    #[expect(clippy::needless_pass_by_ref_mut)] // compat with web api
    pub fn start(&mut self) -> Result<(), LoginError> {
        webbrowser::open(self.callback_server.get_login_url())
            .map_err(|source| LoginError::OpenBrowser { source })
    }

    pub fn open(_egui_ctx: &egui::Context) -> Result<Self, LoginError> {
        let pkce = Pkce::new();

        // Whenever the modal is open, we always keep the callback server running:
        let callback_server = OauthCallbackServer::new(&pkce)
            .map_err(|source| LoginError::StartCallbackServer { source })?;

        Ok(Self {
            callback_server,
            pkce,
            pending_authentication: false,
            credentials: Default::default(),
            show_open_feedback: false,
            show_copy_feedback: false,
        })
    }
}
