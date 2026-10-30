//! The agent panel: the conversation list, or one conversation.

use std::sync::Arc;

use re_agent::acp::schema::v1::{SessionModeId, SessionModeState};
use re_agent::{AgentToolbox, ModelApiProvider};
use re_async::{AsyncRuntimeHandle, UiWaker};
use re_ui::alert::Alert;
use re_ui::{ContextExt as _, UiExt as _, icons};

use super::Screen;
use super::conversation::Conversation;
use super::conversation_list::{ConversationRow, ConversationStatus, conversation_list_ui};
use super::setup_ui::selected_agent_ready;
use re_agent::AgentEntry;
use re_agent::Transcript;
use re_agent::{AgentLaunch, AgentLaunchError};
use re_agent::{AgentSession, Phase, Prompt, TurnReport};
use re_agent::{AgentSettings, SessionContext};

/// How wide a host should make the panel.
///
/// Narrower and the transcript's code blocks and tables are unreadable; wider and the chat
/// takes room from what the agent is there to talk about. Snapshot tests render at this width,
/// so what they show is what a default install looks like.
pub const RECOMMENDED_WIDTH: f32 = 420.0;

/// Shown when hovering "Self-improve" in the options menu.
const SELF_IMPROVE_TOOLTIP: &str = "Rerun development builds only.\n\n\
    Opens a second agent in the Rerun checkout that reviews this session, finds what the \
    viewer's MCP tools, skills, docs, and instructions made hard, and files the fixes.";

/// Goes back to the conversation list, with the input focused to start a new conversation.
///
/// `COMMAND` is Cmd on Mac and Ctrl everywhere else, so this reads as the platform's own
/// new-tab shortcut. Only fires while the panel holds the pointer or the keyboard focus.
pub const NEW_CONVERSATION_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::T);

/// Identifies one conversation for as long as the panel lives. Never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConversationId(u64);

/// What the panel shows: the conversation list, or one conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    List,
    Conversation(ConversationId),
}

/// What the header, the options menu, or the conversation list asked for this frame.
///
/// Variants with a [`ConversationId`] act on that conversation.
#[derive(Clone, Copy)]
pub enum PanelAction {
    ShowList,
    NewConversation,
    Open(ConversationId),
    Close(ConversationId),
    OpenSetup(ConversationId),
    BackToChat(ConversationId),
    Restart(ConversationId),
    SendFeedback,
    SelfImprove,
}

/// The "…" menu at the right end of the header, with the actions for the shown conversation.
struct OptionsMenu {
    /// The conversation the menu acts on.
    conversation: ConversationId,

    /// Agent setup and restart only make sense from the chat.
    in_chat: bool,

    /// See [`AgentPanel::set_feedback_enabled`].
    feedback: bool,

    /// See [`AgentPanel::set_self_improve_available`].
    self_improve: bool,

    /// The shown conversation can be closed. Its row only shows a close button on hover.
    closable: bool,
}

impl OptionsMenu {
    fn ui(self, ui: &mut egui::Ui) -> Option<PanelAction> {
        let button = ui.small_icon_button_widget(&icons::MORE, "More options");
        let menu = egui::containers::menu::MenuButton::from_button(button)
            .config(egui::containers::menu::MenuConfig::new().style(re_ui::menu::menu_style()));
        let (_, inner) = menu.ui(ui, |ui| {
            let action = self.items_ui(ui);
            // The menu only closes by itself on a pointer click, not on keyboard activation.
            if action.is_some() {
                ui.close();
            }
            action
        });
        inner?.inner
    }

    fn items_ui(&self, ui: &mut egui::Ui) -> Option<PanelAction> {
        let &Self {
            conversation,
            in_chat,
            feedback,
            closable,
            self_improve,
        } = self;

        let new_conversation = egui::Button::new("New conversation")
            .shortcut_text(re_ui::format_shortcut(ui.ctx(), &NEW_CONVERSATION_SHORTCUT));
        if ui.add(new_conversation).clicked() {
            return Some(PanelAction::NewConversation);
        }
        if in_chat {
            if ui.button("Agent setup…").clicked() {
                return Some(PanelAction::OpenSetup(conversation));
            }
            if ui
                .button("Clear session and restart agent")
                .on_hover_text("Starts a new session, without the conversation so far")
                .clicked()
            {
                return Some(PanelAction::Restart(conversation));
            }
        }
        if closable && ui.button("Close conversation").clicked() {
            return Some(PanelAction::Close(conversation));
        }
        if feedback || self_improve {
            ui.separator();
        }
        if feedback
            && ui
                .button("Send feedback…")
                .on_hover_text("Tell us how the agent did in this conversation")
                .clicked()
        {
            return Some(PanelAction::SendFeedback);
        }
        if self_improve {
            let badge = egui::RichText::new(" dev ")
                .small()
                .color(ui.tokens().alert_warning.icon)
                .background_color(ui.tokens().alert_warning.fill);
            if ui
                .add(egui::Button::new("Self-improve").shortcut_text(badge))
                .on_hover_text(SELF_IMPROVE_TOOLTIP)
                .clicked()
            {
                return Some(PanelAction::SelfImprove);
            }
        }
        None
    }
}

/// The complete agent UI: a list of conversations. Owns the sessions and the shared settings.
pub struct AgentPanel {
    settings: AgentSettings,

    /// Where model-API agents, such as the `anthropic-api` profile, run.
    ///
    /// Without it they fail to start; subprocess agents need none.
    async_runtime: Option<AsyncRuntimeHandle>,

    /// Set by the host. The tools model-API agents get.
    toolbox: Option<Arc<dyn AgentToolbox>>,

    /// The model-API providers whose key was found in the environment.
    available_model_apis: Vec<ModelApiProvider>,

    context: SessionContext,
    agents: Vec<AgentEntry>,

    /// Every conversation the user has started, in the order they were opened.
    conversations: Vec<Conversation>,

    /// Reports of the turns of closed conversations, until the host takes them.
    closed_conversation_turns: Vec<TurnReport>,

    /// The conversation the list view's input starts. It joins [`Self::conversations`] when the
    /// user sends its first prompt.
    draft: Conversation,

    /// The user opened the draft's setup screen from the options menu, so it fills the panel.
    ///
    /// A draft can also be on its setup screen because its agent failed to start. Then the
    /// conversation list is shown instead, and the error waits until the user opens setup.
    user_opened_draft_setup: bool,

    view: View,
    auto_approve: bool,

    /// Offered in an empty conversation. Set by the host.
    example_prompts: Vec<String>,

    /// Set by the host. Shows "Self-improve" in the options menu.
    self_improve_available: bool,

    /// Whether "Self-improve" was picked during the last [`Self::ui`].
    self_improve_requested: bool,

    /// Set by the host. Shows "Send feedback…" in the options menu.
    feedback_enabled: bool,

    /// Whether "Send feedback…" was picked during the last [`Self::ui`].
    feedback_requested: bool,

    /// Given to the next conversation that is opened.
    next_conversation_id: ConversationId,
}

impl AgentPanel {
    /// A panel for the agents installed on this machine.
    ///
    /// Model-API agents, such as the `anthropic-api` profile, run on `async_runtime`.
    pub fn new(settings: AgentSettings, async_runtime: AsyncRuntimeHandle) -> Self {
        let agents = cfg_select! {
            agent_harness_backend => AgentEntry::detect_installed(),
            _ => Vec::new(),
        };
        let available_model_apis = ModelApiProvider::ALL
            .into_iter()
            .filter(|provider| provider.api_key_from_env().is_some())
            .collect();
        Self::with_agents(settings, Some(async_runtime), agents, available_model_apis)
    }

    /// Like [`Self::new`], but with explicit lists of agents and model APIs with a key, instead
    /// of probing the machine.
    ///
    /// The first run shows the setup screen with an installed agent preselected, so the user
    /// sees which of their local agent CLIs the panel is about to run before it starts.
    /// With no agent installed, a model API is preselected instead, one with a key if there is
    /// one, so the setup screen says what is missing.
    ///
    /// Without an `async_runtime`, model-API agents fail to start.
    pub fn with_agents(
        mut settings: AgentSettings,
        async_runtime: Option<AsyncRuntimeHandle>,
        agents: Vec<AgentEntry>,
        available_model_apis: Vec<ModelApiProvider>,
    ) -> Self {
        if settings.profile_id.is_empty() {
            settings.profile_id = match AgentEntry::pick_default(&agents) {
                Some(agent) => agent.profile.id.clone(),
                None => super::setup_ui::default_model_api(&available_model_apis).profile_id(),
            };
        }

        let mut draft =
            Conversation::new(&settings, async_runtime.clone(), ConversationId(0), false);
        draft.available_model_apis.clone_from(&available_model_apis);
        Self {
            settings,
            async_runtime,
            toolbox: None,
            available_model_apis,
            context: SessionContext::default(),
            agents,
            conversations: Vec::new(),
            closed_conversation_turns: Vec::new(),
            draft,
            user_opened_draft_setup: false,
            view: View::List,
            auto_approve: false,
            example_prompts: Vec::new(),
            self_improve_available: false,
            self_improve_requested: false,
            feedback_enabled: false,
            feedback_requested: false,
            next_conversation_id: ConversationId(1),
        }
    }

    /// The tools that model-API agents, such as the `anthropic-api` profile, may call.
    /// Applies to sessions started from now on. Agent harnesses get their tools from MCP servers.
    pub fn set_toolbox(&mut self, toolbox: Arc<dyn AgentToolbox>) {
        for conversation in self.all_conversations_mut() {
            conversation.toolbox = Some(Arc::clone(&toolbox));
        }
        self.toolbox = Some(toolbox);
    }

    /// What every new session is told about the host. Applies to sessions started from now on.
    pub fn set_context(&mut self, context: SessionContext) {
        self.context = context;
    }

    /// The prompts an empty conversation offers to start from, e.g. depending on what the host
    /// is showing.
    pub fn set_example_prompts(&mut self, prompts: Vec<String>) {
        self.example_prompts = prompts;
    }

    /// Shows "Self-improve" in the options menu, marked as a developer feature. Off by default.
    ///
    /// Only for Rerun's own development builds: the host acts on [`Self::self_improve_requested`]
    /// by starting a side conversation that reviews this one.
    pub fn set_self_improve_available(&mut self, available: bool) {
        self.self_improve_available = available;
    }

    /// Whether "Self-improve" was picked during the last [`Self::ui`].
    pub fn self_improve_requested(&self) -> bool {
        self.self_improve_requested
    }

    /// Shows "Send feedback…" in the options menu while the shown conversation is in chat
    /// and has a finished turn. Off by default.
    pub fn set_feedback_enabled(&mut self, enabled: bool) {
        self.feedback_enabled = enabled;
    }

    /// Whether "Send feedback…" was picked during the last [`Self::ui`].
    ///
    /// The feedback is for the conversation of [`Self::session`].
    pub fn feedback_requested(&self) -> bool {
        self.feedback_requested
    }

    /// What the user chose in setup, shared by all conversations.
    pub fn settings(&self) -> &AgentSettings {
        &self.settings
    }

    /// See [`Self::settings`].
    pub fn settings_mut(&mut self) -> &mut AgentSettings {
        &mut self.settings
    }

    /// The session of the shown conversation, or of the draft while the list is shown.
    pub fn session(&self) -> &AgentSession {
        &self.active_conversation().session
    }

    /// See [`Self::session`].
    pub fn session_mut(&mut self) -> &mut AgentSession {
        &mut self.active_conversation_mut().session
    }

    /// Give the shown conversation's prompt input keyboard focus when it is next shown.
    pub fn request_input_focus(&mut self) {
        self.active_conversation_mut().input.request_focus();
    }

    /// Switch the shown conversation to the chat view. Starts its agent if none is running.
    pub fn show_chat(&mut self) {
        self.active_conversation_mut().screen = Screen::Chat;
    }

    /// The transcript of the shown conversation.
    pub fn transcript(&self) -> &Transcript {
        self.session().transcript()
    }

    /// Whether the shown conversation's agent is in the middle of a turn.
    ///
    /// Its transcript is incomplete until the turn ends, so a host that dumps one wants to know.
    pub fn is_turn_in_progress(&self) -> bool {
        self.session().turn_in_progress()
    }

    /// See [`AgentSession::set_auto_approve`]. Applies to every conversation, current and future.
    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
        for conversation in self.all_conversations_mut() {
            conversation.session.set_auto_approve(auto_approve);
        }
    }

    /// Whether the shown conversation is connected to its agent.
    pub fn is_connected(&self) -> bool {
        self.session().is_connected()
    }

    /// Whether the shown conversation's agent has a session open and is idle.
    pub fn is_ready(&self) -> bool {
        self.session().is_ready()
    }

    /// The permission modes the shown conversation's agent offers, if it offers any.
    pub fn modes(&self) -> Option<&SessionModeState> {
        self.session().modes()
    }

    /// Switches the shown conversation's permission mode, and makes it the default for new ones.
    pub fn set_mode(&mut self, mode_id: SessionModeId) {
        self.settings.preferred_mode = Some(mode_id.clone());
        self.session_mut().set_mode(mode_id);
    }

    /// Sends a prompt in the shown conversation as if the user had typed it.
    /// Returns `false` if it was dropped because the agent is not [`Self::is_ready`].
    pub fn send_prompt(&mut self, prompt: impl Into<Prompt>) -> bool {
        self.session_mut().send_prompt(prompt.into())
    }

    /// Queues a prompt for the shown conversation, to send once its agent is ready.
    ///
    /// The prompt stays with that conversation through startup and setup. Blank input is ignored.
    pub fn queue_prompt(&mut self, prompt: String) {
        if !prompt.trim().is_empty() {
            let conversation = self.active_conversation_mut();
            conversation.pending_prompts.push_back(prompt.into());
            conversation.input.request_focus();
        }
    }

    /// What the current settings would launch, for hosts that run side sessions with the same
    /// agent.
    pub fn agent_launch(&self, context: &SessionContext) -> Result<AgentLaunch, AgentLaunchError> {
        self.settings.launch(&self.agents, context)
    }

    /// The agent that the current settings would launch, to redact with.
    pub fn redaction_agent(&self) -> Result<re_agent::RedactionAgent, AgentLaunchError> {
        Ok(re_agent::RedactionAgent {
            launch: self.agent_launch(&SessionContext::default())?,
            async_runtime: self.async_runtime.clone(),
        })
    }

    /// Drains the reports of the turns finished in every conversation since the last call,
    /// closed ones included.
    pub fn take_finished_turns(&mut self) -> Vec<TurnReport> {
        let mut turns = std::mem::take(&mut self.closed_conversation_turns);
        turns.extend(
            self.all_conversations_mut()
                .flat_map(Conversation::take_finished_turns),
        );
        turns
    }

    /// Starts (or restarts) the agent of the shown conversation with the current settings.
    pub fn start(&mut self, waker: UiWaker) {
        self.start_conversation(waker, self.active_conversation().id);
    }

    /// Starts (or restarts) the agent of any conversation, including the draft.
    fn start_conversation(&mut self, waker: UiWaker, id: ConversationId) {
        let Self {
            settings,
            context,
            agents,
            conversations,
            draft,
            ..
        } = self;
        if let Some(conversation) =
            std::iter::chain(conversations.iter_mut(), std::iter::once(draft))
                .find(|conversation| conversation.id == id)
        {
            conversation.start(waker, settings, context, agents);
        }
    }

    /// Opens a blank conversation, with its input focused.
    ///
    /// It is the draft, which becomes a conversation of its own when the user sends its first
    /// prompt.
    pub fn new_conversation(&mut self) {
        self.view = View::Conversation(self.draft.id);
        if self.user_opened_draft_setup {
            self.user_opened_draft_setup = false;
            if *self.draft.session.phase() != Phase::Idle {
                self.draft.screen = Screen::Chat;
            }
        }
        self.draft.input.request_focus();
    }

    /// Opens a conversation that runs with `context` instead of the panel's shared one, and sends
    /// `opening_prompt` as soon as its agent is ready.
    ///
    /// For work the host starts on the user's behalf in a session of its own: a different working
    /// directory, preamble, or set of readable directories than the chat the user is having.
    pub fn open_conversation(
        &mut self,
        context: SessionContext,
        opening_prompt: impl Into<String>,
    ) {
        let mut conversation = self.fresh_conversation();
        conversation.context = Some(context);
        conversation
            .pending_prompts
            .push_back(Prompt::from(opening_prompt.into()));
        self.view = View::Conversation(conversation.id);
        self.conversations.push(conversation);
    }

    /// A new conversation under a new id, for the current settings.
    fn fresh_conversation(&mut self) -> Conversation {
        let id = self.next_conversation_id;
        self.next_conversation_id = ConversationId(id.0 + 1);
        let mut conversation = Conversation::new(
            &self.settings,
            self.async_runtime.clone(),
            id,
            self.auto_approve,
        );
        conversation.toolbox.clone_from(&self.toolbox);
        conversation
            .available_model_apis
            .clone_from(&self.available_model_apis);
        conversation
    }

    /// Once the draft has a prompt, it becomes a conversation of its own, and is shown.
    fn promote_started_draft(&mut self) {
        let draft_shown = self.view == View::List || self.view == View::Conversation(self.draft.id);
        if !draft_shown || !self.draft.has_started() {
            return;
        }
        let next = self.fresh_conversation();
        let mut started = std::mem::replace(&mut self.draft, next);
        self.user_opened_draft_setup = false;
        started.mark_seen();
        // It is shown under its own id from now on, so the input loses the focus it had.
        started.input.request_focus();
        self.view = View::Conversation(started.id);
        self.conversations.push(started);
    }

    /// Keeps the unused draft in line with the settings.
    ///
    /// It skips setup once setup is done in another conversation. It is replaced when it runs
    /// another agent than the selected one, unless the selected agent cannot be launched or the
    /// draft's own setup is open. What the user typed into it carries over.
    fn refresh_draft(&mut self) {
        // A draft whose start failed keeps its setup screen, with the error in the transcript.
        let draft = &mut self.draft;
        if draft.screen == Screen::Setup
            && !self.user_opened_draft_setup
            && self.settings.setup_done
            && *draft.session.phase() == Phase::Idle
            && draft.session.transcript().items.is_empty()
        {
            draft.screen = Screen::Chat;
        }

        let outdated = self.draft.profile_id != self.settings.profile_id
            && *self.draft.session.phase() != Phase::Idle
            && !self.draft.has_started()
            && !self.user_opened_draft_setup
            && selected_agent_ready(&self.settings, &self.agents, &self.available_model_apis)
                .is_ok();
        if outdated {
            let next = self.fresh_conversation();
            let mut outdated = std::mem::replace(&mut self.draft, next);
            self.draft.input = std::mem::take(&mut outdated.input);
        }
    }

    /// Pump every conversation's agent connection.
    ///
    /// [`Self::ui`] does this too, but must also be called while the panel is collapsed,
    /// or messages queue up unseen until the user opens it again.
    pub fn poll_events(&mut self) {
        for conversation in self.all_conversations_mut() {
            conversation.session.poll_events();
            conversation.send_pending_prompts();
        }
        self.promote_started_draft();
    }

    /// Shows the header, then the conversation list or the shown conversation.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_events();
        self.refresh_draft();

        let mut action = self.header_ui(ui);
        let panel_rect = ui.max_rect();

        let Self {
            settings,
            context,
            agents,
            conversations,
            draft,
            user_opened_draft_setup,
            view,
            example_prompts,
            ..
        } = self;
        match *view {
            View::Conversation(id) => {
                if let Some(conversation) = conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == id)
                {
                    ui.push_id(id, |ui| {
                        conversation.ui(ui, settings, context, agents, example_prompts, None);
                    });
                    // Its setup screen hides the transcript.
                    if conversation.screen == Screen::Chat {
                        conversation.mark_seen();
                    }
                } else if id == draft.id {
                    ui.push_id(id, |ui| {
                        draft.ui(ui, settings, context, agents, example_prompts, None);
                    });
                } else {
                    *view = View::List;
                }
            }
            View::List if conversations.is_empty() => {
                ui.push_id(draft.id, |ui| {
                    draft.ui(ui, settings, context, agents, example_prompts, None);
                });
            }
            View::List => {
                let rows = conversation_rows(conversations, settings, agents);
                // The draft has no row, so the list is the only place its startup error can go.
                let draft_error = match draft.status() {
                    ConversationStatus::Errored { message } => Some(message),
                    _ => None,
                };
                let draft_id = draft.id;
                let mut list_action = None;
                let mut home_ui = |ui: &mut egui::Ui| {
                    if let Some(message) = &draft_error {
                        list_action = draft_error_ui(ui, message, draft_id);
                    }
                    let picked = conversation_list_ui(ui, &rows);
                    list_action = list_action.or(picked);
                };
                if draft.screen == Screen::Setup && !*user_opened_draft_setup {
                    egui::CentralPanel::default().show(ui, |ui| home_ui(ui));
                } else {
                    ui.push_id(draft.id, |ui| {
                        draft.ui(
                            ui,
                            settings,
                            context,
                            agents,
                            example_prompts,
                            Some(&mut home_ui),
                        );
                    });
                }
                action = action.or(list_action);
            }
        }

        if new_conversation_shortcut_pressed(ui, panel_rect) {
            action = Some(PanelAction::NewConversation);
        }
        self.handle_action(ui.ctx(), action);
        self.promote_started_draft();
    }

    /// The bar along the top: back button and title, and the options menu.
    fn header_ui(&self, ui: &mut egui::Ui) -> Option<PanelAction> {
        let tokens = ui.tokens();
        let conversation = self.active_conversation();
        let in_conversation = matches!(self.view, View::Conversation(_));
        let title = if in_conversation {
            conversation.title(&self.settings, &self.agents)
        } else {
            "Rerun agent".to_owned()
        };
        let in_chat = conversation.screen == Screen::Chat;
        // From the list, the draft's setup leads back to the list, even if its agent never ran.
        let can_leave_setup = *conversation.session.phase() != Phase::Idle
            || !in_conversation && !self.conversations.is_empty();
        let menu = OptionsMenu {
            conversation: conversation.id,
            in_chat,
            feedback: self.feedback_enabled && in_chat && conversation.session.has_finished_turn(),
            // It reviews the shown conversation, which the list does not have.
            self_improve: self.self_improve_available && in_conversation,
            closable: in_conversation,
        };

        let height = tokens.top_bar_height();
        egui::Panel::top("agent_header")
            .exact_size(height)
            .frame(
                egui::Frame::new()
                    .fill(tokens.tab_bar_color)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| {
                let (left, right) = egui::containers::Sides::new()
                    .height(height)
                    .shrink_left()
                    .truncate()
                    .show(
                        ui,
                        |ui| {
                            let back = in_conversation
                                && ui
                                    .small_icon_button(&icons::ARROW_LEFT, "Back to conversations")
                                    .on_hover_text("Back to conversations")
                                    .clicked();
                            ui.label(egui::RichText::new(title).strong());
                            back.then_some(PanelAction::ShowList)
                        },
                        |ui| {
                            let action = menu.ui(ui);
                            let back_to_chat = !in_chat
                                && can_leave_setup
                                && ui
                                    .small_icon_button(&icons::CLOSE, "Back to the conversation")
                                    .on_hover_text("Back to the conversation")
                                    .clicked();
                            if back_to_chat {
                                Some(PanelAction::BackToChat(conversation.id))
                            } else {
                                action
                            }
                        },
                    );
                left.or(right)
            })
            .inner
    }

    fn handle_action(&mut self, ctx: &egui::Context, action: Option<PanelAction>) {
        self.self_improve_requested = false;
        self.feedback_requested = false;
        let Some(action) = action else {
            return;
        };
        match action {
            PanelAction::ShowList => self.view = View::List,
            PanelAction::NewConversation => self.new_conversation(),
            PanelAction::Open(id) => self.view = View::Conversation(id),
            PanelAction::Close(id) => {
                if let Some(index) = self
                    .conversations
                    .iter()
                    .position(|conversation| conversation.id == id)
                {
                    let closed = self.conversations.remove(index);
                    self.closed_conversation_turns
                        .extend(closed.into_finished_turns());
                }
                if self.view == View::Conversation(id) {
                    self.view = View::List;
                }
            }
            PanelAction::OpenSetup(id) => {
                self.user_opened_draft_setup |= id == self.draft.id;
                if let Some(conversation) = self.conversation_mut(id) {
                    conversation.screen = Screen::Setup;
                }
            }
            PanelAction::BackToChat(id) => {
                if id == self.draft.id {
                    self.user_opened_draft_setup = false;
                }
                if let Some(conversation) = self.conversation_mut(id) {
                    conversation.screen = Screen::Chat;
                }
            }
            PanelAction::Restart(id) => self.start_conversation(ctx.ui_waker(), id),
            PanelAction::SendFeedback => self.feedback_requested = true,
            PanelAction::SelfImprove => self.self_improve_requested = true,
        }
    }

    /// The shown conversation, or the draft while the list is shown.
    fn active_conversation(&self) -> &Conversation {
        match self.view {
            View::Conversation(id) => self
                .conversations
                .iter()
                .find(|conversation| conversation.id == id)
                .unwrap_or(&self.draft),
            View::List => &self.draft,
        }
    }

    fn active_conversation_mut(&mut self) -> &mut Conversation {
        match self.view {
            View::Conversation(id) => self
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
                .unwrap_or(&mut self.draft),
            View::List => &mut self.draft,
        }
    }

    /// Any conversation, including the draft.
    fn conversation_mut(&mut self, id: ConversationId) -> Option<&mut Conversation> {
        self.all_conversations_mut()
            .find(|conversation| conversation.id == id)
    }

    fn all_conversations_mut(&mut self) -> impl Iterator<Item = &mut Conversation> {
        std::iter::chain(
            self.conversations.iter_mut(),
            std::iter::once(&mut self.draft),
        )
    }
}

/// The rows of the conversation list: by status, then the most recently prompted first.
/// Why the draft's agent failed to start, and a way to fix its setup.
fn draft_error_ui(ui: &mut egui::Ui, message: &str, draft: ConversationId) -> Option<PanelAction> {
    let action = Alert::error()
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(message);
            ui.button("Agent setup…")
                .clicked()
                .then_some(PanelAction::OpenSetup(draft))
        })
        .inner;
    ui.add_space(8.0);
    action
}

fn conversation_rows(
    conversations: &[Conversation],
    settings: &AgentSettings,
    agents: &[AgentEntry],
) -> Vec<ConversationRow> {
    let mut rows: Vec<ConversationRow> = conversations
        .iter()
        .map(|conversation| ConversationRow {
            id: conversation.id,
            title: conversation.title(settings, agents),
            status: conversation.status(),
            last_prompt_at: conversation.last_prompt_at(),
        })
        .collect();
    rows.sort_by_key(|row| (row.status.rank(), std::cmp::Reverse(row.last_prompt_at)));
    rows
}

/// Whether the user pressed [`NEW_CONVERSATION_SHORTCUT`] "in" the panel.
///
/// The panel is one widget among many in its host, so the shortcut is scoped to it: it only
/// counts while the pointer is over the panel or a widget inside it has keyboard focus.
fn new_conversation_shortcut_pressed(ui: &egui::Ui, panel_rect: egui::Rect) -> bool {
    let ctx = ui.ctx();

    let pointer_inside = ctx
        .pointer_latest_pos()
        .is_some_and(|pos| panel_rect.contains(pos));
    let focus_inside = ctx
        .memory(|memory| memory.focused())
        .and_then(|id| ctx.read_response(id))
        .is_some_and(|response| panel_rect.contains_rect(response.rect));

    (pointer_inside || focus_inside)
        && ui.input_mut(|input| input.consume_shortcut(&NEW_CONVERSATION_SHORTCUT))
}

#[cfg(test)]
mod tests {
    use re_agent::AgentProfileId;

    use super::super::conversation_list::ConversationStatus;
    use super::*;

    #[test]
    fn turn_that_ends_unseen_is_a_notification() {
        let mut conversation =
            Conversation::new(&AgentSettings::default(), None, ConversationId(0), false);
        conversation
            .session
            .transcript_mut()
            .push_user("Hello".into());
        conversation.session.begin_test_turn();
        conversation.mark_seen();
        assert!(conversation.status() == ConversationStatus::Working);

        conversation
            .session
            .handle_event(re_agent::AgentEvent::TurnFinished(Ok(
                re_agent::acp::schema::v1::StopReason::EndTurn,
            )));
        assert!(conversation.status() == ConversationStatus::Notification { needs_input: false });

        conversation.mark_seen();
        assert!(conversation.status() == ConversationStatus::NothingNew);
    }

    /// Checks that prompts from the host join the busy session's queue before later chat prompts.
    #[test]
    fn host_prompts_join_busy_session_queue() {
        let mut panel =
            AgentPanel::with_agents(AgentSettings::default(), None, Vec::new(), Vec::new());
        let session = panel.session_mut();
        session.handle_event(re_agent::AgentEvent::SessionStarted {
            session_id: re_agent::acp::schema::v1::SessionId::new("test-session"),
            modes: None,
            config_options: Vec::new(),
        });
        session.begin_test_turn();

        panel.queue_prompt("Prompt from command palette".to_owned());
        panel.poll_events();
        let session = panel.session_mut();
        assert!(session.send_prompt("Prompt from chat".into()));
        assert_eq!(
            session
                .queued_prompts()
                .iter()
                .map(|prompt| prompt.text.as_str())
                .collect::<Vec<_>>(),
            ["Prompt from command palette", "Prompt from chat"],
        );
    }

    /// Checks that prompts survive setup and remain attached to the conversation they were sent to.
    #[test]
    fn host_prompts_survive_setup_and_stay_with_their_conversation() {
        let mut panel =
            AgentPanel::with_agents(AgentSettings::default(), None, Vec::new(), Vec::new());
        panel.queue_prompt("First prompt".to_owned());
        panel.queue_prompt("Second prompt".to_owned());
        panel.queue_prompt("   ".to_owned());
        panel.poll_events();
        panel.open_conversation(SessionContext::default(), "Other conversation");
        panel.poll_events();

        let pending = |conversation: &Conversation| {
            conversation
                .pending_prompts
                .iter()
                .map(|prompt| prompt.text.clone())
                .collect::<Vec<_>>()
        };
        let [first, other] = panel.conversations.as_slice() else {
            panic!(
                "expected two conversations, got {}",
                panel.conversations.len()
            );
        };
        assert_eq!(first.screen, Screen::Setup);
        assert_eq!(pending(first), ["First prompt", "Second prompt"]);
        assert_eq!(pending(other), ["Other conversation"]);
        assert_eq!(panel.view, View::Conversation(other.id));
    }

    /// Picking another agent replaces an unused, running draft, but only with an agent that can
    /// be launched.
    #[test]
    fn draft_follows_the_selected_agent_only_if_it_is_installed() {
        let agents = re_agent::AgentProfile::builtin()
            .into_iter()
            .map(|profile| {
                let installed = matches!(profile.id.as_str(), "claude" | "codex");
                AgentEntry {
                    executable: installed
                        .then(|| std::path::PathBuf::from("/usr/local/bin").join(&profile.command)),
                    profile,
                }
            })
            .collect();
        let settings = AgentSettings {
            profile_id: AgentProfileId::new("claude"),
            ..Default::default()
        };
        let mut panel = AgentPanel::with_agents(settings, None, agents, Vec::new());
        panel
            .draft
            .session
            .handle_event(re_agent::AgentEvent::SessionStarted {
                session_id: re_agent::acp::schema::v1::SessionId::new("test-session"),
                modes: None,
                config_options: Vec::new(),
            });
        panel.draft.input.prompt = "Half-written".into();
        let first = panel.draft.id;

        panel.settings.profile_id = AgentProfileId::new("gemini");
        panel.refresh_draft();
        assert_eq!(panel.draft.id, first, "gemini is not installed");

        panel.settings.profile_id = AgentProfileId::new("codex");
        panel.user_opened_draft_setup = true;
        panel.refresh_draft();
        assert_eq!(panel.draft.id, first, "the draft's own setup starts codex");

        panel.user_opened_draft_setup = false;
        panel.refresh_draft();
        assert_ne!(panel.draft.id, first, "codex is installed");
        assert_eq!(panel.draft.input.prompt.text, "Half-written");
    }

    /// Setup finished in another conversation, e.g. one the host opened, applies to the draft.
    #[test]
    fn draft_skips_setup_once_another_conversation_finished_it() {
        let mut panel =
            AgentPanel::with_agents(AgentSettings::default(), None, Vec::new(), Vec::new());
        panel.refresh_draft();
        assert_eq!(panel.draft.screen, Screen::Setup, "setup is not done yet");

        panel.settings.setup_done = true;
        panel.draft.session.report_error("Failed to start");
        panel.refresh_draft();
        assert_eq!(panel.draft.screen, Screen::Setup, "its own start failed");

        panel.draft = Conversation::new(&AgentSettings::default(), None, ConversationId(99), false);
        panel.refresh_draft();
        assert_eq!(panel.draft.screen, Screen::Chat);
    }

    #[test]
    fn draft_becomes_a_conversation_once_it_has_a_prompt() {
        let mut panel =
            AgentPanel::with_agents(AgentSettings::default(), None, Vec::new(), Vec::new());
        panel.poll_events();
        assert!(panel.conversations.is_empty());
        assert_eq!(panel.view, View::List);

        panel.queue_prompt("Hello".to_owned());
        panel.poll_events();
        let [conversation] = panel.conversations.as_slice() else {
            panic!(
                "expected one conversation, got {}",
                panel.conversations.len()
            );
        };
        assert_eq!(panel.view, View::Conversation(conversation.id));
        assert!(!panel.draft.has_started());
    }
}
