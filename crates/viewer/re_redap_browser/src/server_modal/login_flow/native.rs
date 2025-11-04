use std::time::Duration;

use re_auth::{callback_server::OauthCallbackServer, oauth::Credentials};
use re_ui::icons;

use super::ActionButton;

pub struct State {
    callback_server: OauthCallbackServer,

    show_open_feedback: bool,
    show_copy_feedback: bool,
}

impl State {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // The native login flow consists of having the user open a link in their browser,
        // which eventually sends back the credentials to the `callback_server`.
        ui.horizontal(|ui| {
            if ActionButton::primary(&icons::EXTERNAL_LINK, "Login", "Link opened!")
                .show(ui, &mut self.show_open_feedback)
                .clicked()
            {
                webbrowser::open(self.callback_server.get_login_url()).ok();
            }

            if ActionButton::secondary(&icons::COPY, "Copy link", "Copied to clipboard!")
                .show(ui, &mut self.show_copy_feedback)
                .clicked()
            {
                ui.ctx()
                    .copy_text(self.callback_server.get_login_url().to_owned());
            }
        });

        ui.ctx().request_repaint_after(Duration::from_millis(10));
    }

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn done(&mut self) -> Result<Option<Credentials>, String> {
        // We're done if we received valid credentials from the client:
        match self.callback_server.check_for_browser_response() {
            Ok(Some(response)) => {
                #[expect(unsafe_code)]
                // SAFETY: credentials come from a trusted source
                let credentials = unsafe { Credentials::from_auth_response(response.into()) }
                    .map_err(|e| e.to_string())?;
                let credentials = credentials.ensure_stored().map_err(|e| e.to_string())?;
                Ok(Some(credentials))
            }
            Ok(None) => Ok(None),
            Err(err) => Err(format!("Failed to check for browser response: {err}")),
        }
    }

    pub fn open(_ui: &mut egui::Ui) -> Result<Self, String> {
        // Whenever the modal is open, we always keep the callback server running:
        let callback_server = OauthCallbackServer::new(None)
            .map_err(|err| format!("Failed to start callback server: {err}"))?;

        Ok(Self {
            callback_server,
            show_open_feedback: false,
            show_copy_feedback: false,
        })
    }
}
