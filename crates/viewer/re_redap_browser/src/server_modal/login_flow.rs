#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

use egui::{IntoAtoms as _, vec2};
#[cfg(not(target_arch = "wasm32"))]
use native::State;
use re_auth::oauth::Credentials;
use re_ui::UiExt as _;
use re_ui::notifications::{Notification, NotificationLevel};
use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};
#[cfg(target_arch = "wasm32")]
use web::State;

pub struct LoginFlow {
    state: State,
    #[cfg(target_arch = "wasm32")]
    started: bool,
}

pub enum LoginFlowResult {
    Success(Credentials),
    Failure(String),
}

impl LoginFlow {
    pub fn open(ui: &mut egui::Ui, login_hint: Option<&str>) -> Result<Self, String> {
        State::open(ui, login_hint).map(|state| Self {
            state,
            #[cfg(target_arch = "wasm32")]
            started: false,
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, cmd: &CommandSender) -> Option<LoginFlowResult> {
        #[cfg(target_arch = "wasm32")]
        {
            if !self.started {
                // Show button to start the flow
                if ActionButton::primary(&re_ui::icons::EXTERNAL_LINK, "Login", "Login")
                    .show(ui, &mut false)
                    .clicked()
                {
                    if let Err(err) = self.state.start() {
                        return Some(LoginFlowResult::Failure(err));
                    }
                    self.started = true;
                }
                None
            } else {
                // Show spinner while waiting
                self.state.ui(ui);
                self.done(ui, cmd)
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // On native, always show the buttons
            self.state.ui(ui);
            self.done(ui, cmd)
        }
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

            Err(err) => Some(LoginFlowResult::Failure(err)),
        }
    }
}

#[derive(Clone, Copy)]
enum ActionButtonStyle {
    Primary,
    Secondary,
}

pub struct ActionButton<'a> {
    icon: &'a re_ui::Icon,
    action_text: &'a str,
    feedback_text: &'a str,
    style: ActionButtonStyle,
}

impl<'a> ActionButton<'a> {
    pub fn primary(icon: &'a re_ui::Icon, action_text: &'a str, feedback_text: &'a str) -> Self {
        Self {
            icon,
            action_text,
            feedback_text,
            style: ActionButtonStyle::Primary,
        }
    }

    #[cfg_attr(target_arch = "wasm32", expect(dead_code))] // only used on native
    pub fn secondary(icon: &'a re_ui::Icon, action_text: &'a str, feedback_text: &'a str) -> Self {
        Self {
            icon,
            action_text,
            feedback_text,
            style: ActionButtonStyle::Secondary,
        }
    }

    pub fn show(&self, ui: &mut egui::Ui, show_feedback: &mut bool) -> egui::Response {
        let response = ui
            .scope(|ui| {
                let tokens = ui.tokens();
                let visuals = &mut ui.style_mut().visuals;

                if matches!(self.style, ActionButtonStyle::Primary) {
                    visuals.override_text_color = Some(tokens.text_inverse);
                }

                let spacing = &mut ui.style_mut().spacing;
                spacing.button_padding = vec2(5.0, 4.0);

                let response = ui.ctx().read_response(ui.next_auto_id());
                let fill_color = match self.style {
                    ActionButtonStyle::Primary => {
                        if response.is_some_and(|r| r.hovered()) {
                            tokens.bg_fill_inverse_hover
                        } else {
                            tokens.bg_fill_inverse
                        }
                    }
                    ActionButtonStyle::Secondary => {
                        if response.is_some_and(|r| r.hovered()) {
                            tokens.widget_active_bg_fill
                        } else {
                            tokens.widget_noninteractive_bg_stroke
                        }
                    }
                };

                let label = if *show_feedback {
                    self.feedback_text
                } else {
                    self.action_text
                };

                let icon_tint = match self.style {
                    ActionButtonStyle::Primary => tokens.icon_inverse,
                    ActionButtonStyle::Secondary => tokens.list_item_default_icon,
                };
                let icon = self
                    .icon
                    .as_image()
                    .tint(icon_tint)
                    .fit_to_exact_size(vec2(16.0, 16.0));
                let atoms = (egui::Atom::grow(), label, icon, egui::Atom::grow()).into_atoms();

                ui.add(egui::Button::new(atoms).fill(fill_color).corner_radius(3.0))
            })
            .inner;

        if response.clicked() {
            *show_feedback = true;
        } else if !response.hovered() {
            *show_feedback = false;
        }

        response
    }
}
