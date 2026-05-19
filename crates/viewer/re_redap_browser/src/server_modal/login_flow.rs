#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native::State;
use re_ui::ReButton;
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
    Success,
    Failure(String),
}

impl LoginFlow {
    /// Open a new login flow.
    ///
    /// `signed_in_url` is only used on web as the `OAuth` redirect URI.
    /// On native, it is ignored (the callback server handles redirects).
    pub fn open(egui_ctx: &egui::Context, signed_in_url: Option<&str>) -> Result<Self, String> {
        let _ = &signed_in_url; // only used on web

        #[cfg(target_arch = "wasm32")]
        let state = State::open(
            egui_ctx,
            signed_in_url
                .ok_or("signed_in_url is required for web login")?
                .to_owned(),
        )?;
        #[cfg(not(target_arch = "wasm32"))]
        let state = State::open(egui_ctx)?;
        Ok(Self {
            state,
            #[cfg(target_arch = "wasm32")]
            started: false,
        })
    }

    /// Create and immediately start the login flow (opens popup on web, opens browser on native).
    ///
    /// `signed_in_url` is only used on web as the `OAuth` redirect URI.
    /// On native, it is ignored (the callback server handles redirects).
    pub fn open_and_start(
        egui_ctx: &egui::Context,
        signed_in_url: Option<&str>,
    ) -> Result<Self, String> {
        let mut flow = Self::open(egui_ctx, signed_in_url)?;
        flow.state.start()?;
        #[cfg(target_arch = "wasm32")]
        {
            flow.started = true;
        }
        Ok(flow)
    }

    /// Poll for completion without rendering any UI.
    pub fn poll(&mut self) -> Option<LoginFlowResult> {
        match self.state.done() {
            Ok(Some(_credentials)) => Some(LoginFlowResult::Success),
            Ok(None) => None,
            Err(err) => Some(LoginFlowResult::Failure(err)),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, cmd: &CommandSender) -> Option<LoginFlowResult> {
        #[cfg(target_arch = "wasm32")]
        {
            if !self.started {
                // Show button to start the flow
                if ActionButton::new(&re_ui::icons::EXTERNAL_LINK, "Log in", "Log in")
                    .variant(re_ui::Variant::Outlined)
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
                // Show loading indicator while waiting
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
                ui.request_repaint();

                cmd.send_system(SystemCommand::ShowNotification(Notification::new(
                    NotificationLevel::Success,
                    format!("Logged in as {}", credentials.user().email),
                )));

                Some(LoginFlowResult::Success)
            }

            Ok(None) => None,

            Err(err) => Some(LoginFlowResult::Failure(err)),
        }
    }
}

pub struct ActionButton<'a> {
    icon: &'a re_ui::Icon,
    action_text: &'a str,
    feedback_text: &'a str,
    variant: re_ui::Variant,
}

impl<'a> ActionButton<'a> {
    pub fn new(icon: &'a re_ui::Icon, action_text: &'a str, feedback_text: &'a str) -> Self {
        Self {
            icon,
            action_text,
            feedback_text,
            variant: re_ui::Variant::default(),
        }
    }

    pub fn variant(mut self, style: re_ui::Variant) -> Self {
        self.variant = style;
        self
    }

    pub fn show(&self, ui: &mut egui::Ui, show_feedback: &mut bool) -> egui::Response {
        let label = if *show_feedback {
            self.feedback_text
        } else {
            self.action_text
        };

        let response = ui.add(
            ReButton::new((label, self.icon))
                .variant(self.variant)
                .small(),
        );

        if response.clicked() {
            *show_feedback = true;
        } else if !response.hovered() {
            *show_feedback = false;
        }

        response
    }
}
