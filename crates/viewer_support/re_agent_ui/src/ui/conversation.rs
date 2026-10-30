//! One conversation: an agent session with its input, what the user has seen of it, and its
//! setup screen.

use std::collections::VecDeque;
use std::sync::Arc;

use re_agent::{
    AgentEntry, AgentLaunch, AgentProfileId, AgentSession, AgentSettings, AgentToolbox,
    ModelApiProvider, Phase, Prompt, SessionContext, TurnReport,
};
use re_async::{AsyncRuntimeHandle, UiWaker};
use re_ui::UiExt as _;
use web_time::SystemTime;

use super::Screen;
use super::agent_panel::ConversationId;
use super::chat_ui::{ChatInput, chat_ui};
use super::conversation_list::ConversationStatus;
use super::setup_ui::{selected_agent_name, setup_ui};

/// One chat with one agent: a row in the conversation list.
pub struct Conversation {
    /// Unique for each conversation.
    pub id: ConversationId,

    /// Where a model-API agent runs. Without it, a model-API agent fails to start.
    pub async_runtime: Option<AsyncRuntimeHandle>,

    /// The tools a model-API agent gets. See [`super::AgentPanel::set_toolbox`].
    pub toolbox: Option<Arc<dyn AgentToolbox>>,

    /// The model-API providers whose key was found, offered on the setup screen.
    pub available_model_apis: Vec<ModelApiProvider>,

    /// When the conversation was opened, for sorting until it has any messages.
    created_at: SystemTime,

    /// How many transcript entries the user has seen in their final form, for the unseen-message
    /// dot.
    ///
    /// Mid-turn, the last entry can still grow, so it does not count as seen.
    seen_finished_items: usize,

    /// The connection to its agent, and the transcript.
    pub session: AgentSession,

    /// Reports of the turns of sessions that a restart replaced, until the host takes them.
    replaced_session_turns: Vec<TurnReport>,

    /// The prompt being written.
    pub input: ChatInput,

    /// The agent profile it last started, which can differ from the one now selected in setup.
    pub profile_id: AgentProfileId,

    /// [`Screen::Chat`] with an idle session means "start the agent on the next frame".
    pub screen: Screen,

    /// Overrides the panel's shared context. See [`super::AgentPanel::open_conversation`].
    pub context: Option<SessionContext>,

    /// Prompts waiting for the agent to be ready, e.g. sent from the command palette.
    pub pending_prompts: VecDeque<Prompt>,
}

impl Conversation {
    /// A fresh conversation. Skips the setup screen when the user has completed it before.
    pub fn new(
        settings: &AgentSettings,
        async_runtime: Option<AsyncRuntimeHandle>,
        id: ConversationId,
        auto_approve: bool,
    ) -> Self {
        let mut session = AgentSession::default();
        session.set_auto_approve(auto_approve);
        Self {
            id,
            async_runtime,
            toolbox: None,
            available_model_apis: Vec::new(),
            created_at: SystemTime::now(),
            seen_finished_items: 0,
            session,
            replaced_session_turns: Vec::new(),
            input: ChatInput::default(),
            profile_id: settings.profile_id.clone(),
            screen: if settings.setup_done {
                Screen::Chat
            } else {
                Screen::Setup
            },
            context: None,
            pending_prompts: Default::default(),
        }
    }

    /// Starts (or restarts) its agent, or goes to setup with the error if it cannot be launched.
    ///
    /// Marks setup as done in `settings` on success.
    pub fn start(
        &mut self,
        waker: UiWaker,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &[AgentEntry],
    ) {
        let context = self.context.as_ref().unwrap_or(context);
        match settings.launch(agents, context) {
            Ok(mut launch) => {
                if let AgentLaunch::ModelApi(config) = &mut launch {
                    config.toolbox.clone_from(&self.toolbox);
                }
                self.profile_id.clone_from(&settings.profile_id);
                // Shut the old agent down before the new one starts.
                let replaced = std::mem::take(&mut self.session);
                let auto_approve = replaced.auto_approve();
                self.replaced_session_turns
                    .extend(replaced.into_finished_turns());

                self.session = AgentSession::start(launch, self.async_runtime.as_ref(), waker);
                self.session.set_auto_approve(auto_approve);
                self.seen_finished_items = 0;
                self.input.reset_keeping_prompt();
                self.screen = Screen::Chat;
                settings.setup_done = true;
            }
            Err(err) => {
                self.session.report_error(err.to_string());
                self.screen = Screen::Setup;
            }
        }
    }

    /// Drains the reports of the turns finished since the last call, including those of sessions
    /// that a restart replaced.
    pub fn take_finished_turns(&mut self) -> Vec<TurnReport> {
        let mut turns = std::mem::take(&mut self.replaced_session_turns);
        turns.extend(self.session.take_finished_turns());
        turns
    }

    /// Ends the conversation, and returns the reports of every turn the host has not taken yet,
    /// including the running turn, reported as aborted.
    pub fn into_finished_turns(self) -> Vec<TurnReport> {
        let Self {
            session,
            mut replaced_session_turns,
            ..
        } = self;
        replaced_session_turns.extend(session.into_finished_turns());
        replaced_session_turns
    }

    /// Session title if the agent gave one, else the first line of the first prompt, else the
    /// agent name.
    pub fn title(&self, settings: &AgentSettings, agents: &[AgentEntry]) -> String {
        let transcript = self.session.transcript();
        transcript
            .title
            .clone()
            .or_else(|| {
                let prompt = transcript
                    .items
                    .iter()
                    .find_map(|entry| entry.item.user_prompt())?;
                Some(prompt.text.lines().next().unwrap_or_default().to_owned())
            })
            .or_else(|| self.session.agent_name().map(ToOwned::to_owned))
            .or_else(|| selected_agent_name(settings, agents))
            .unwrap_or_else(|| "New chat".to_owned())
    }

    /// Whether the user has asked anything yet, so that it belongs in the conversation list.
    pub fn has_started(&self) -> bool {
        let has_prompt = self
            .session
            .transcript()
            .items
            .iter()
            .any(|entry| entry.item.user_prompt().is_some());
        has_prompt || !self.pending_prompts.is_empty() || self.session.turn_in_progress()
    }

    /// What the conversation list shows for it.
    pub fn status(&self) -> ConversationStatus {
        let session = &self.session;
        let items = &session.transcript().items;
        let needs_input = session.is_waiting_on_user();
        let last_error = items.last().and_then(|entry| entry.item.error_text());
        if needs_input {
            ConversationStatus::Notification { needs_input: true }
        } else if let Phase::Disconnected { reason } = session.phase() {
            ConversationStatus::Errored {
                message: reason.clone(),
            }
        } else if let Some(message) = last_error {
            ConversationStatus::Errored {
                message: message.to_owned(),
            }
        } else if session.turn_in_progress() {
            ConversationStatus::Working
        } else if self.seen_finished_items < items.len() {
            ConversationStatus::Notification { needs_input: false }
        } else {
            ConversationStatus::NothingNew
        }
    }

    /// The user has seen everything in the transcript so far.
    pub fn mark_seen(&mut self) {
        let items = self.session.transcript().items.len();
        self.seen_finished_items = if self.session.turn_in_progress() {
            items.saturating_sub(1)
        } else {
            items
        };
    }

    /// When the user last sent a message, or else when the conversation was opened.
    pub fn last_prompt_at(&self) -> SystemTime {
        self.session
            .transcript()
            .items
            .iter()
            .rev()
            .find(|entry| entry.item.user_prompt().is_some())
            .map_or(self.created_at, |entry| entry.created_at)
    }

    /// Shows setup or the chat, and starts the agent once the chat is shown.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &mut [AgentEntry],
        example_prompts: &[String],
        home_ui: Option<&mut dyn FnMut(&mut egui::Ui)>,
    ) {
        if self.screen == Screen::Chat && *self.session.phase() == Phase::Idle {
            self.start(ui.ui_waker(), settings, context, agents);
        }

        if self.screen == Screen::Setup {
            self.setup_view(ui, settings, context, agents);
            return;
        }

        let login_hint = agents
            .iter()
            .find(|agent| agent.profile.id == self.profile_id)
            .map(|agent| agent.profile.login_hint.as_str())
            .filter(|hint| !hint.is_empty());
        let mut preferred_model = settings.preferred_models.get(&self.profile_id).cloned();
        chat_ui(
            ui,
            &mut self.session,
            &mut self.input,
            &mut settings.preferred_mode,
            &mut preferred_model,
            settings.show_thoughts,
            login_hint,
            example_prompts,
            home_ui,
        );
        if let Some(model) = preferred_model {
            settings
                .preferred_models
                .insert(self.profile_id.clone(), model);
        }
        if let Some(prompt) = self.input.example_prompt_clicked.take() {
            self.pending_prompts.push_back(prompt.into());
            self.input.request_focus();
        }
        if self.input.restart_after_login_requested {
            // Restarting after login clears the transcript, so resend the failed prompt.
            if let Some(prompt) = self
                .session
                .transcript()
                .items
                .iter()
                .rev()
                .find_map(|entry| entry.item.user_prompt().cloned())
            {
                self.pending_prompts.push_front(prompt);
            }
            settings.profile_id.clone_from(&self.profile_id);
            self.start(ui.ui_waker(), settings, context, agents);
        }
    }

    /// Sends queued prompts once the agent can take them.
    ///
    /// Call this every frame: a refused prompt stays queued for the next call.
    pub fn send_pending_prompts(&mut self) {
        while let Some(prompt) = self.pending_prompts.pop_front() {
            if !self.session.send_prompt(prompt.clone()) {
                self.pending_prompts.push_front(prompt);
                break;
            }
        }
    }

    fn setup_view(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &mut [AgentEntry],
    ) {
        let back = *self.session.phase() != Phase::Idle
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let next_screen = egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                setup_ui(ui, settings, agents, &self.available_model_apis)
            })
            .inner;

        if next_screen == Screen::Chat {
            self.start(ui.ui_waker(), settings, context, agents);
        } else if back {
            self.screen = Screen::Chat;
        }
    }
}
