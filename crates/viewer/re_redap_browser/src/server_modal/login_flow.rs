#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

use egui::{IntoAtoms as _, vec2};
use re_auth::oauth::Credentials;
use re_ui::{
    UiExt as _,
    notifications::{Notification, NotificationLevel},
};
use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};

#[cfg(not(target_arch = "wasm32"))]
use native::State;
#[cfg(target_arch = "wasm32")]
use web::State;

pub struct LoginFlow {
    state: State,
}

pub enum LoginFlowResult {
    Success(Credentials),
    Failure(String),
}

impl LoginFlow {
    pub fn open(ui: &mut egui::Ui) -> Result<Self, String> {
        match State::open(ui) {
            Ok(state) => Ok(Self { state }),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, cmd: &CommandSender) -> Option<LoginFlowResult> {
        self.state.ui(ui);
        self.done(ui, cmd)
    }

    fn done(&mut self, ui: &egui::Ui, cmd: &CommandSender) -> Option<LoginFlowResult> {
        match self.state.done() {
            Ok(Some(credentials)) => {
                ui.ctx().request_repaint();

                cmd.send_system(SystemCommand::ShowNotification(Notification::new(
                    NotificationLevel::Success,
                    format!("Logged in as {}", credentials.user().email),
                )));

                Some(LoginFlowResult::Success(credentials))
            }

            Ok(None) => None,

            Err(err) => Some(LoginFlowResult::Failure(err.to_string())),
        }
    }
}

pub fn action_button(
    ui: &mut egui::Ui,
    show_feedback: &mut bool,
    icon: Option<&re_ui::Icon>,
    action_text: &str,
    feedback_text: &str,
) -> bool {
    let response = ui
        .scope(|ui| {
            let tokens = ui.tokens();
            let visuals = &mut ui.style_mut().visuals;
            visuals.override_text_color = Some(tokens.text_inverse);

            let spacing = &mut ui.style_mut().spacing;
            spacing.button_padding = vec2(16.0, 6.0);

            let response = ui.ctx().read_response(ui.next_auto_id());
            let fill_color = if response.is_some_and(|r| r.hovered()) {
                tokens.bg_fill_inverse_hover
            } else {
                tokens.bg_fill_inverse
            };

            let label = if *show_feedback {
                feedback_text
            } else {
                action_text
            };
            let icon = icon.map(|icon| {
                icon.as_image()
                    .tint(ui.tokens().icon_inverse)
                    .fit_to_exact_size(vec2(16.0, 16.0))
            });
            let atoms = match icon {
                Some(icon) => (egui::Atom::grow(), icon, label, egui::Atom::grow()).into_atoms(),
                None => (egui::Atom::grow(), label, egui::Atom::grow()).into_atoms(),
            };

            ui.add(egui::Button::new(atoms).fill(fill_color))
        })
        .inner;

    if response.clicked() {
        *show_feedback = true;
        return true;
    } else if !response.hovered() {
        *show_feedback = false;
    }

    false
}
