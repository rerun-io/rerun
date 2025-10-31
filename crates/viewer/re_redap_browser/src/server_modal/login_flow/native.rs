use std::time::Duration;

use re_auth::{
    callback_server::{self, OauthCallbackServer},
    oauth::{Credentials, CredentialsStoreError, MalformedTokenError},
};
use re_ui::icons;

use super::action_button;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to start callback server: {0}")]
    CallbackServer(#[from] callback_server::Error),

    #[error(transparent)]
    MalformedToken(#[from] MalformedTokenError),

    #[error(transparent)]
    CredentialsStore(#[from] CredentialsStoreError),
}

pub struct State {
    callback_server: OauthCallbackServer,

    show_open_feedback: bool,
    show_copy_feedback: bool,
}

impl State {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            let mut url_for_text_edit = self.callback_server.get_login_url().to_owned();
            egui::TextEdit::singleline(&mut url_for_text_edit)
                .hint_text("<can't share link>") // No known way to get into this situation.
                .text_color(ui.style().visuals.strong_text_color())
                .desired_width(f32::INFINITY) // Take up the entire space.
                .show(ui);

            ui.horizontal(|ui| {
                if action_button(
                    ui,
                    &mut self.show_open_feedback,
                    Some(&icons::EXTERNAL_LINK),
                    "Open in browser",
                    "Link opened!",
                ) {
                    webbrowser::open(self.callback_server.get_login_url()).ok();
                }

                if action_button(
                    ui,
                    &mut self.show_copy_feedback,
                    Some(&icons::URL),
                    "Copy URL",
                    "Copied to clipboard!",
                ) {
                    ui.ctx()
                        .copy_text(self.callback_server.get_login_url().to_owned());
                }
            });
        });

        ui.ctx().request_repaint_after(Duration::from_millis(10));
    }

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn done(&mut self) -> Result<Option<Credentials>, Error> {
        match self.callback_server.check_for_browser_response() {
            Ok(Some(response)) => {
                #[expect(unsafe_code)]
                // SAFETY: credentials come from a trusted source
                let credentials = unsafe { Credentials::from_auth_response(response.into())? };
                let credentials = credentials.ensure_stored()?;
                Ok(Some(credentials))
            }
            Ok(None) => Ok(None),
            Err(err) => Err(Error::CallbackServer(err)),
        }
    }

    pub fn open(_ui: &mut egui::Ui) -> Result<Self, Error> {
        let callback_server = OauthCallbackServer::new(None)?;

        Ok(Self {
            callback_server,
            show_open_feedback: false,
            show_copy_feedback: false,
        })
    }
}
