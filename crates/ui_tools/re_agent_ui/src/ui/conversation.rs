//! One conversation: an agent session with its input, what the user has seen of it, and its
//! setup screen.

use std::collections::VecDeque;
use std::sync::Arc;

use re_agent::{
    AgentEntry, AgentLaunch, AgentPreferences, AgentProfileId, AgentSession, AgentSettings,
    AgentToolbox, ModelApiProvider, Phase, Prompt, SavedConversation, SessionContext,
    SessionReports,
};
use re_async::{AsyncRuntimeHandle, UiWaker};
use re_ui::UiExt as _;
use web_time::SystemTime;

use super::Screen;
use super::agent_panel::ConversationId;
use super::chat_ui::{ChatInput, chat_ui};
use super::conversation_list::ConversationStatus;
use super::setup_ui::{agent_name, selected_agent_name, setup_ui};

/// One chat with one agent: a row in the conversation list.
pub struct Conversation {
    /// Unique for each conversation.
    pub id: ConversationId,

    /// Where a model-API agent runs. Without it, a model-API agent fails to start.
    pub async_runtime: Option<AsyncRuntimeHandle>,

    /// The tools a model-API agent gets, and that an agent harness may reach over MCP.
    /// See [`super::AgentPanel::set_toolbox`].
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

    /// Reports of sessions that a restart replaced, until the host takes them.
    replaced_session_reports: SessionReports,

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

    /// What [`Self::save`] last kept of it. Every start of a harness asks the agent to load it,
    /// so that neither a restart of the viewer nor of the agent loses the conversation.
    saved: Option<SavedConversation>,
}

impl Conversation {
    /// A fresh conversation. Skips the setup screen when the user has completed it before.
    pub fn new(
        settings: &AgentSettings,
        async_runtime: Option<AsyncRuntimeHandle>,
        id: ConversationId,
    ) -> Self {
        let session = AgentSession::default();
        Self {
            id,
            async_runtime,
            toolbox: None,
            available_model_apis: Vec::new(),
            created_at: SystemTime::now(),
            seen_finished_items: 0,
            session,
            replaced_session_reports: SessionReports::default(),
            input: ChatInput::default(),
            profile_id: settings.profile_id.clone(),
            screen: if settings.setup_done {
                Screen::Chat
            } else {
                Screen::Setup
            },
            context: None,
            pending_prompts: Default::default(),
            saved: None,
        }
    }

    /// A conversation from an earlier run. Its agent starts, and loads it, once it is shown.
    pub fn restored(
        settings: &AgentSettings,
        async_runtime: Option<AsyncRuntimeHandle>,
        id: ConversationId,
        saved: SavedConversation,
    ) -> Self {
        let mut conversation = Self::new(settings, async_runtime, id);
        conversation.created_at = to_system_time(saved.last_prompt_at);
        conversation.profile_id.clone_from(&saved.profile_id);
        conversation.screen = Screen::Chat;
        conversation.saved = Some(saved);
        conversation
    }

    /// What to keep of it for the next run, if its agent can load it again.
    ///
    /// Conversations the host opened with a context of its own are not kept, since that context
    /// is gone after a restart.
    pub fn save(
        &mut self,
        settings: &AgentSettings,
        agents: &[AgentEntry],
    ) -> Option<SavedConversation> {
        re_tracing::profile_function!();
        let savable = self.context.is_none() && self.profile_id.model_api_provider().is_none();
        if savable
            && self.has_started()
            && let Some(session_id) = self.session.session_id()
        {
            self.saved = Some(SavedConversation {
                profile_id: self.profile_id.clone(),
                session_id: session_id.clone(),
                cwd: self.session.cwd().to_owned(),
                title: self.title(settings, agents),
                last_prompt_at: to_timestamp(self.last_prompt_at()),
            });
        }
        self.saved.clone()
    }

    /// Starts (or restarts) its agent, or goes to setup with the error if it cannot be launched.
    ///
    /// Continues the saved conversation, if there is one, with the agent that had it.
    /// Marks setup as done in `settings` on success.
    pub fn start(
        &mut self,
        waker: UiWaker,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &[AgentEntry],
    ) {
        let context = self.context.as_ref().unwrap_or(context);
        let launched = if let Some(saved) = &self.saved
            && saved.profile_id != settings.profile_id
        {
            let mut settings = settings.clone();
            settings.profile_id.clone_from(&saved.profile_id);
            settings.launch(agents, context)
        } else {
            settings.launch(agents, context)
        };
        match launched {
            Ok(mut launch) => {
                match &mut launch {
                    AgentLaunch::AgentHarness(config) => {
                        if let Some(saved) = &self.saved {
                            config.session_to_load = Some(saved.session_id.clone());
                            let cwd_exists = cfg_select! {
                                target_arch = "wasm32" => false,
                                _ => saved.cwd.is_dir(),
                            };
                            if cwd_exists {
                                config.cwd.clone_from(&saved.cwd);
                            }
                        }
                    }
                    AgentLaunch::ModelApi(config) => {
                        config.toolbox.clone_from(&self.toolbox);
                    }
                }
                self.profile_id = self
                    .saved
                    .as_ref()
                    .map_or(&settings.profile_id, |saved| &saved.profile_id)
                    .clone();
                // Shut the old agent down before the new one starts.
                let replaced = std::mem::take(&mut self.session);
                let auto_approve = replaced.auto_approve();
                self.replaced_session_reports
                    .append(replaced.into_reports());

                self.session = AgentSession::start(launch, self.async_runtime.as_ref(), waker);
                self.session.set_auto_approve(auto_approve);
                self.session.set_toolbox(self.toolbox.clone());
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

    /// Drains what its sessions reported since the last call, including sessions that a restart
    /// replaced.
    pub fn take_reports(&mut self) -> SessionReports {
        let mut reports = std::mem::take(&mut self.replaced_session_reports);
        reports.append(self.session.take_reports());
        reports
    }

    /// Ends the conversation, and returns every report the host has not taken yet, including the
    /// running turn, reported as aborted.
    pub fn into_reports(self) -> SessionReports {
        let Self {
            session,
            mut replaced_session_reports,
            ..
        } = self;
        replaced_session_reports.append(session.into_reports());
        replaced_session_reports
    }

    /// Session title if the agent gave one, else the first line of the first prompt, else the
    /// saved title, else the agent name.
    ///
    /// The saved title only stands in until the agent has replayed the history.
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
            .or_else(|| self.saved.as_ref().map(|saved| saved.title.clone()))
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
    ///
    /// For a restored conversation, its saved time stands in for the replayed history.
    pub fn last_prompt_at(&self) -> SystemTime {
        self.session
            .transcript()
            .items
            .iter()
            .skip(self.session.history_len())
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
        let mut preferences = settings
            .preferences(&self.profile_id)
            .cloned()
            .unwrap_or_default();
        let agent_name = agent_name(&self.profile_id, agents)
            .or_else(|| self.session.agent_name().map(ToOwned::to_owned))
            .unwrap_or_else(|| "Custom agent".to_owned());
        chat_ui(
            ui,
            &mut self.session,
            &mut self.input,
            &mut preferences,
            Some(&agent_name),
            settings.show_thoughts,
            login_hint,
            example_prompts,
            home_ui,
        );
        if preferences != AgentPreferences::default() {
            *settings.preferences_mut(self.profile_id.clone()) = preferences;
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
            // The user picked an agent here, which cannot load another agent's session.
            if self
                .saved
                .as_ref()
                .is_some_and(|saved| saved.profile_id != settings.profile_id)
            {
                self.saved = None;
            }
            self.start(ui.ui_waker(), settings, context, agents);
        } else if back {
            self.screen = Screen::Chat;
        }
    }
}

fn to_system_time(timestamp: jiff::Timestamp) -> SystemTime {
    let since_epoch = timestamp.duration_since(jiff::Timestamp::UNIX_EPOCH);
    SystemTime::UNIX_EPOCH + std::time::Duration::try_from(since_epoch).unwrap_or_default()
}

fn to_timestamp(time: SystemTime) -> jiff::Timestamp {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|since_epoch| jiff::Timestamp::UNIX_EPOCH.checked_add(since_epoch).ok())
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
}
