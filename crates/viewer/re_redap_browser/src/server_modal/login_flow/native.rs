use std::sync::Arc;
use std::time::Duration;

use egui::mutex::Mutex;
use re_auth::callback_server::OauthCallbackServer;
use re_auth::oauth::Credentials;
use re_auth::oauth::api::{AuthenticateWithCode, Pkce, send_native};
use re_ui::{UiExt as _, Variant, icons};

use super::ActionButton;

pub struct State {
    callback_server: OauthCallbackServer,

    pkce: Pkce,
    pending_authentication: bool,
    credentials: Arc<Mutex<Option<Result<Credentials, String>>>>,

    show_open_feedback: bool,
    show_copy_feedback: bool,
}

impl State {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // The native login flow uses oauth 2.0 code authorization flow with PKCE

        if self.pending_authentication {
            ui.loading_indicator();
        } else {
            ui.horizontal(|ui| {
                if ActionButton::new(&icons::EXTERNAL_LINK, "Log in", "Link opened!")
                    .variant(Variant::Outlined)
                    .show(ui, &mut self.show_open_feedback)
                    .clicked()
                {
                    webbrowser::open(self.callback_server.get_login_url()).ok();
                }

                if ActionButton::new(&icons::COPY, "Copy link", "Copied to clipboard!")
                    .show(ui, &mut self.show_copy_feedback)
                    .clicked()
                {
                    ui.ctx()
                        .copy_text(self.callback_server.get_login_url().to_owned());
                }
            });
        }

        ui.ctx().request_repaint_after(Duration::from_millis(10));
    }

    pub fn done(&mut self) -> Result<Option<Credentials>, String> {
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
                let on_done = move |res: Result<Credentials, String>| {
                    *credentials.lock() = Some(res);
                };
                send_native(
                    AuthenticateWithCode::new(&code, &self.pkce),
                    move |res| match res {
                        Ok(res) => {
                            let credentials = match Credentials::from_auth_response(res.into())
                                .map_err(|err| err.to_string())
                            {
                                Ok(c) => c,
                                Err(err) => return on_done(Err(err)),
                            };
                            let credentials =
                                match credentials.ensure_stored().map_err(|err| err.to_string()) {
                                    Ok(c) => c,
                                    Err(err) => return on_done(Err(err)),
                                };

                            on_done(Ok(credentials));
                        }
                        Err(res) => on_done(Err(res.to_string())),
                    },
                );
                self.pending_authentication = true;

                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(err) => Err(format!("Failed to check for browser response: {err}")),
        }
    }

    pub fn open(_ui: &mut egui::Ui) -> Result<Self, String> {
        let pkce = Pkce::new();

        // Whenever the modal is open, we always keep the callback server running:
        let callback_server = OauthCallbackServer::new(&pkce)
            .map_err(|err| format!("Failed to start callback server: {err}"))?;

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
