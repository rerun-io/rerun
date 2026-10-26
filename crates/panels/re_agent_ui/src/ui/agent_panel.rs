//! The complete agent panel: one tab per conversation, each showing setup until its agent runs,
//! then the chat.

use std::collections::VecDeque;

use egui_tiles::{Container, Tile, TileId, Tiles};
use re_agent::acp::schema::v1::{SessionModeId, SessionModeState};
use re_ui::{UiExt as _, icons};

use super::Screen;
use super::chat_ui::{ChatInput, chat_ui};
use super::setup_ui::setup_ui;
use re_agent::AgentEntry;
use re_agent::{AgentSession, Phase, Prompt, TurnReport};
use re_agent::{AgentSettings, LaunchConfig, SessionContext};
use re_agent::{Transcript, TranscriptItem};

/// How wide a host should make the panel.
///
/// Narrower and the transcript's code blocks and tables are unreadable; wider and the chat
/// takes room from what the agent is there to talk about. Snapshot tests render at this width,
/// so what they show is what a default install looks like.
pub const RECOMMENDED_WIDTH: f32 = 420.0;

/// Longest tab title before it is cut with an ellipsis.
const MAX_TAB_TITLE_CHARS: usize = 24;

/// Shown when hovering "Self-improve" in the options menu.
const SELF_IMPROVE_TOOLTIP: &str = "Rerun development builds only.\n\n\
    Opens a second agent in the Rerun checkout that reviews this session, finds what the \
    viewer's MCP tools, skills, docs, and instructions made hard, and files the fixes.";

/// Horizontal padding inside a tab, around its title.
const TAB_TITLE_SPACING: f32 = 12.0;

/// Gap between the tabs and the buttons at either end of the tab bar.
const TAB_BAR_BUTTON_SPACING: f32 = 8.0;

/// Opens another conversation tab, the way a browser opens another tab.
///
/// `COMMAND` is Cmd on Mac and Ctrl everywhere else, so this reads as the platform's own
/// new-tab shortcut. Only fires while the panel holds the pointer or the keyboard focus.
const NEW_CONVERSATION_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::T);

/// Identifies one conversation for as long as the panel lives. Never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConversationId(u64);

/// One chat with one agent: the content of a tab.
struct Conversation {
    id: ConversationId,
    session: AgentSession,
    input: ChatInput,
    profile_id: String,

    /// [`Screen::Chat`] with an idle session means "start the agent on the next frame".
    screen: Screen,

    /// Used instead of the panel's shared context, for a conversation the host started with a
    /// job of its own: a different working directory, preamble, or set of readable directories.
    context: Option<SessionContext>,

    /// Prompts waiting for the agent to be ready, e.g. sent from the command palette.
    ///
    /// Use `push_back()` to enqueue, and `pop_front()` to get the next pending prompt.
    pending_prompts: VecDeque<Prompt>,
}

impl Conversation {
    /// A fresh conversation. Skips the setup screen when the user has completed it before.
    fn new(id: ConversationId, settings: &AgentSettings, auto_approve: bool) -> Self {
        let mut session = AgentSession::default();
        session.set_auto_approve(auto_approve);
        Self {
            id,
            session,
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

    fn start(
        &mut self,
        ctx: &egui::Context,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &[AgentEntry],
    ) {
        let context = self.context.as_ref().unwrap_or(context);
        match settings.launch_config(agents, context) {
            Ok(config) => {
                self.profile_id.clone_from(&settings.profile_id);
                let ctx = ctx.clone();
                self.session.start(config, move || ctx.request_repaint());
                self.input = ChatInput::default();
                self.screen = Screen::Chat;
                settings.setup_done = true;
            }
            Err(err) => {
                self.session.report_error(err);
                self.screen = Screen::Setup;
            }
        }
    }

    /// Session title if the agent gave one, else the first prompt, else the agent name.
    fn title(&self, settings: &AgentSettings, agents: &[AgentEntry]) -> String {
        let transcript = self.session.transcript();
        let title = transcript
            .title
            .clone()
            .or_else(|| {
                transcript.items.iter().find_map(|entry| match &entry.item {
                    TranscriptItem::User(prompt) => {
                        Some(prompt.text.lines().next().unwrap_or_default().to_owned())
                    }
                    _ => None,
                })
            })
            .or_else(|| self.session.agent_name().map(ToOwned::to_owned))
            .or_else(|| {
                agents
                    .iter()
                    .find(|agent| agent.profile.id == settings.profile_id)
                    .map(|agent| agent.profile.name.clone())
            })
            .unwrap_or_else(|| "New chat".to_owned());

        if title.chars().count() <= MAX_TAB_TITLE_CHARS {
            title
        } else {
            let cut: String = title.chars().take(MAX_TAB_TITLE_CHARS - 1).collect();
            format!("{}…", cut.trim_end())
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &mut [AgentEntry],
    ) {
        if self.screen == Screen::Chat && *self.session.phase() == Phase::Idle {
            self.start(ui.ctx(), settings, context, agents);
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
            // Restarting after login clears the transcript, so retain
            // the failed prompt and its attachments to resend once the new session is ready.
            if let Some(prompt) = self
                .session
                .transcript()
                .items
                .iter()
                .rev()
                .find_map(|entry| {
                    if let TranscriptItem::User(prompt) = &entry.item {
                        Some(prompt.clone())
                    } else {
                        None
                    }
                })
            {
                // Put the prompt at the front of the queue so it's picked up as the next prompt on restart.
                self.pending_prompts.push_front(prompt);
            }
            settings.profile_id.clone_from(&self.profile_id);
            self.start(ui.ctx(), settings, context, agents);
        }
    }

    /// Sends host prompts once the agent can take them.
    ///
    /// Must be called repeatedly: the agent is not ready for some time after the conversation is
    /// opened, and a send can still be refused after that, so the prompt is kept until one lands.
    /// [`AgentPanel::poll_events`] does this every frame, including while the panel is collapsed.
    fn send_pending_prompts(&mut self) {
        while let Some(prompt) = self.pending_prompts.pop_front() {
            if !self.session.send_prompt(prompt.clone()) {
                // Agent not ready yet; retry on the next poll.
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
        // Escape returns to the conversation, once an agent has been started.
        let back = *self.session.phase() != Phase::Idle
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let next_screen = egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| setup_ui(ui, settings, agents))
            .inner;

        if next_screen == Screen::Chat {
            self.start(ui.ctx(), settings, context, agents);
        } else if back {
            self.screen = Screen::Chat;
        }
    }
}

/// What the tab bar asked for this frame: the "+" button, the options menu, or the back button.
///
/// Variants with a [`ConversationId`] act on that conversation.
#[derive(Clone, Copy)]
enum TabBarAction {
    NewConversation,
    OpenSetup(ConversationId),
    BackToChat(ConversationId),
    Restart(ConversationId),
    SendFeedback,
    SelfImprove,
}

/// Tabs only: no dragging tabs into splits, closable tabs, "+" for a new conversation,
/// and the options menu at the right end of the bar.
struct TabsBehavior<'a> {
    settings: &'a mut AgentSettings,
    context: &'a SessionContext,
    agents: &'a mut Vec<AgentEntry>,
    self_improve_available: bool,
    feedback_enabled: bool,
    action: Option<TabBarAction>,
}

impl egui_tiles::Behavior<Conversation> for TabsBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        conversation: &mut Conversation,
    ) -> egui_tiles::UiResponse {
        conversation.ui(ui, self.settings, self.context, self.agents);
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, conversation: &Conversation) -> egui::WidgetText {
        egui::RichText::new(conversation.title(self.settings, self.agents))
            .strong()
            .into()
    }

    fn is_tab_closable(&self, _tiles: &Tiles<Conversation>, _tile_id: TileId) -> bool {
        true
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Conversation>, tile_id: TileId) -> bool {
        if let Some(Tile::Pane(conversation)) = tiles.get_mut(tile_id) {
            conversation.session.stop();
        }
        true
    }

    fn is_tile_draggable(&self, _tiles: &Tiles<Conversation>, _tile_id: TileId) -> bool {
        false
    }

    fn tab_bar_trailing_ui(
        &mut self,
        _tiles: &Tiles<Conversation>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
    ) {
        ui.add_space(TAB_BAR_BUTTON_SPACING);
        let tooltip = format!(
            "New conversation ({})",
            re_ui::format_shortcut(ui.ctx(), &NEW_CONVERSATION_SHORTCUT)
        );
        if ui
            .small_icon_button(&icons::ADD, "New conversation")
            .on_hover_text(tooltip)
            .clicked()
        {
            self.action = Some(TabBarAction::NewConversation);
        }
    }

    fn top_bar_right_ui(
        &mut self,
        tiles: &Tiles<Conversation>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let Some(active) = tabs.active else {
            return;
        };
        let Some(conversation) = tiles.get_pane(&active) else {
            return;
        };
        let has_agent = *conversation.session.phase() != Phase::Idle;
        let in_chat = conversation.screen == Screen::Chat;

        ui.add_space(TAB_BAR_BUTTON_SPACING);
        let menu = OptionsMenu {
            conversation: conversation.id,
            in_chat,
            feedback: self.feedback_enabled && in_chat && conversation.session.has_finished_turn(),
            self_improve: self.self_improve_available,
        };
        if let Some(action) = menu.ui(ui) {
            self.action = Some(action);
        }

        if !in_chat
            && has_agent
            && ui
                .small_icon_button(&icons::CLOSE, "Back to the conversation")
                .on_hover_text("Back to the conversation")
                .clicked()
        {
            self.action = Some(TabBarAction::BackToChat(conversation.id));
        }
    }

    // Styling, matching the viewport's tabs:

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        re_ui::design_tokens_of_visuals(visuals).tab_bar_color
    }

    fn tab_bg_color(
        &self,
        visuals: &egui::Visuals,
        _tiles: &Tiles<Conversation>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            visuals.panel_fill
        } else {
            self.tab_bar_color(visuals)
        }
    }

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<Conversation>,
        _tile_id: TileId,
        _state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        egui::Stroke::NONE
    }

    fn tab_text_color(
        &self,
        visuals: &egui::Visuals,
        _tiles: &Tiles<Conversation>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        }
    }

    fn tab_title_spacing(&self, _visuals: &egui::Visuals) -> f32 {
        TAB_TITLE_SPACING
    }

    fn tab_bar_height(&self, style: &egui::Style) -> f32 {
        re_ui::design_tokens_of_visuals(&style.visuals).title_bar_height()
    }

    fn dragged_overlay_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        visuals.panel_fill.gamma_multiply(0.5)
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            prune_empty_tabs: false,
            prune_empty_containers: false,
            prune_single_child_tabs: false,
            ..Default::default()
        }
    }
}

/// The "…" menu at the right end of the tab bar, with the actions for the active conversation.
struct OptionsMenu {
    /// The active conversation, which the menu acts on.
    conversation: ConversationId,

    /// Agent setup and restart only make sense from the chat.
    in_chat: bool,

    /// See [`AgentPanel::set_feedback_enabled`].
    feedback: bool,

    /// See [`AgentPanel::set_self_improve_available`].
    self_improve: bool,
}

impl OptionsMenu {
    fn ui(self, ui: &mut egui::Ui) -> Option<TabBarAction> {
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

    fn items_ui(&self, ui: &mut egui::Ui) -> Option<TabBarAction> {
        let &Self {
            conversation,
            in_chat,
            feedback,
            self_improve,
        } = self;

        let new_conversation = egui::Button::new("New conversation")
            .shortcut_text(re_ui::format_shortcut(ui.ctx(), &NEW_CONVERSATION_SHORTCUT));
        if ui.add(new_conversation).clicked() {
            return Some(TabBarAction::NewConversation);
        }
        if in_chat {
            if ui.button("Agent setup…").clicked() {
                return Some(TabBarAction::OpenSetup(conversation));
            }
            if ui
                .button("Clear session and restart agent")
                .on_hover_text("Starts a new session, without the conversation so far")
                .clicked()
            {
                return Some(TabBarAction::Restart(conversation));
            }
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
            return Some(TabBarAction::SendFeedback);
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
                return Some(TabBarAction::SelfImprove);
            }
        }
        None
    }
}

/// The complete agent UI: a tab per conversation. Owns the sessions and the shared settings.
pub struct AgentPanel {
    settings: AgentSettings,
    context: SessionContext,
    agents: Vec<AgentEntry>,
    tree: egui_tiles::Tree<Conversation>,
    auto_approve: bool,

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
    pub fn new(settings: AgentSettings) -> Self {
        Self::with_agents(settings, AgentEntry::detect_all())
    }

    /// Like [`Self::new`], but with an explicit list of agents instead of probing the machine.
    ///
    /// The first run shows the setup screen with an installed agent preselected, so the user
    /// sees which of their local agent CLIs the panel is about to run before it starts.
    pub fn with_agents(mut settings: AgentSettings, agents: Vec<AgentEntry>) -> Self {
        if settings.profile_id.is_empty()
            && let Some(agent) = AgentEntry::pick_default(&agents).or_else(|| agents.first())
        {
            settings.profile_id = agent.profile.id.clone();
        }

        let first = Conversation::new(ConversationId(0), &settings, false);
        Self {
            settings,
            context: SessionContext::default(),
            agents,
            tree: new_tree(first),
            auto_approve: false,
            self_improve_available: false,
            self_improve_requested: false,
            feedback_enabled: false,
            feedback_requested: false,
            next_conversation_id: ConversationId(1),
        }
    }

    /// What every new session is told about the host. Applies to sessions started from now on.
    pub fn set_context(&mut self, context: SessionContext) {
        self.context = context;
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

    /// Shows "Send feedback…" in the options menu while the active conversation is in chat
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

    pub fn settings(&self) -> &AgentSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut AgentSettings {
        &mut self.settings
    }

    /// The session of the active tab.
    pub fn session(&self) -> Option<&AgentSession> {
        self.active_conversation()
            .map(|conversation| &conversation.session)
    }

    pub fn session_mut(&mut self) -> Option<&mut AgentSession> {
        active_conversation_mut(&mut self.tree).map(|conversation| &mut conversation.session)
    }

    /// Give the active conversation's prompt input keyboard focus when it is next shown.
    pub fn request_input_focus(&mut self) {
        if let Some(conversation) = active_conversation_mut(&mut self.tree) {
            conversation.input.request_focus();
        }
    }

    /// Switch the active tab to the chat view. Starts its agent if none is running.
    pub fn show_chat(&mut self) {
        if let Some(conversation) = active_conversation_mut(&mut self.tree) {
            conversation.screen = Screen::Chat;
        }
    }

    /// The transcript of the active tab.
    pub fn transcript(&self) -> Option<&Transcript> {
        self.session().map(AgentSession::transcript)
    }

    /// Whether the active conversation's agent is in the middle of a turn.
    ///
    /// Its transcript is incomplete until the turn ends, so a host that dumps one wants to know.
    pub fn turn_in_progress(&self) -> bool {
        self.session().is_some_and(AgentSession::turn_in_progress)
    }

    /// See [`AgentSession::set_auto_approve`]. Applies to every conversation, current and future.
    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
        for conversation in conversations_mut(&mut self.tree) {
            conversation.session.set_auto_approve(auto_approve);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.session().is_some_and(AgentSession::is_connected)
    }

    pub fn is_ready(&self) -> bool {
        self.session().is_some_and(AgentSession::is_ready)
    }

    pub fn modes(&self) -> Option<&SessionModeState> {
        self.session()?.modes()
    }

    pub fn set_mode(&mut self, mode_id: SessionModeId) {
        self.settings.preferred_mode = Some(mode_id.clone());
        if let Some(session) = self.session_mut() {
            session.set_mode(mode_id);
        }
    }

    /// Sends a prompt in the active tab as if the user had typed it.
    /// Returns `false` if it was dropped because the agent is not [`Self::is_ready`].
    pub fn send_prompt(&mut self, prompt: impl Into<Prompt>) -> bool {
        let prompt = prompt.into();
        self.session_mut()
            .is_some_and(|session| session.send_prompt(prompt))
    }

    /// Submit text to the active conversation, retaining it through startup and setup.
    /// Queued text stays with that conversation when tabs change; blank input is ignored.
    pub fn queue_prompt(&mut self, prompt: String) {
        if !prompt.trim().is_empty()
            && let Some(conversation) = active_conversation_mut(&mut self.tree)
        {
            conversation.pending_prompts.push_back(prompt.into());
            conversation.input.request_focus();
        }
    }

    /// What the current settings would launch, for hosts that run side sessions with the same
    /// agent.
    pub fn launch_config(&self, context: &SessionContext) -> Result<LaunchConfig, String> {
        self.settings.launch_config(&self.agents, context)
    }

    /// Drains the reports of the turns finished in every conversation since the last call.
    pub fn take_finished_turns(&mut self) -> Vec<TurnReport> {
        conversations_mut(&mut self.tree)
            .flat_map(|conversation| conversation.session.take_finished_turns())
            .collect()
    }

    /// Starts (or restarts) the agent of the active tab with the current settings.
    pub fn start(&mut self, ctx: &egui::Context) {
        let Self {
            settings,
            context,
            agents,
            tree,
            ..
        } = self;
        if let Some(conversation) = active_conversation_mut(tree) {
            conversation.start(ctx, settings, context, agents);
        }
    }

    /// Stops the agent of the active tab.
    pub fn stop(&mut self) {
        if let Some(session) = self.session_mut() {
            session.stop();
        }
    }

    /// Opens a new conversation tab and makes it active.
    pub fn new_conversation(&mut self) {
        let conversation = Conversation::new(
            self.allocate_conversation_id(),
            &self.settings,
            self.auto_approve,
        );
        self.insert_conversation(conversation);
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
        let mut conversation = Conversation::new(
            self.allocate_conversation_id(),
            &self.settings,
            self.auto_approve,
        );
        conversation.context = Some(context);
        conversation
            .pending_prompts
            .push_back(Prompt::from(opening_prompt.into()));
        self.insert_conversation(conversation);
    }

    fn allocate_conversation_id(&mut self) -> ConversationId {
        let id = self.next_conversation_id;
        self.next_conversation_id = ConversationId(id.0 + 1);
        id
    }

    /// Adds `conversation` as a tab and makes it active.
    fn insert_conversation(&mut self, conversation: Conversation) {
        let root_tabs = self
            .tree
            .root()
            .and_then(|root| match self.tree.tiles.get(root) {
                Some(Tile::Container(Container::Tabs(_))) => Some(root),
                _ => None,
            });
        match root_tabs {
            Some(root) => {
                let tile_id = self.tree.tiles.insert_pane(conversation);
                if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(root)
                {
                    tabs.add_child(tile_id);
                    tabs.set_active(tile_id);
                }
            }
            None => self.tree = new_tree(conversation),
        }
    }

    /// Pump every conversation's agent connection.
    ///
    /// [`Self::ui`] does this too, but must also be called while the panel is collapsed,
    /// or messages queue up unseen until the user opens it again.
    pub fn poll_events(&mut self) {
        for conversation in conversations_mut(&mut self.tree) {
            conversation.session.poll_events();
            conversation.send_pending_prompts();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_events();

        let mut behavior = TabsBehavior {
            settings: &mut self.settings,
            context: &self.context,
            agents: &mut self.agents,
            self_improve_available: self.self_improve_available,
            feedback_enabled: self.feedback_enabled,
            action: None,
        };
        self.tree.ui(&mut behavior, ui);
        let panel_rect = ui.min_rect();
        self.self_improve_requested = false;
        self.feedback_requested = false;
        let mut add_requested = false;
        match behavior.action {
            Some(TabBarAction::NewConversation) => add_requested = true,
            Some(TabBarAction::OpenSetup(id)) => {
                if let Some(conversation) = conversation_mut(&mut self.tree, id) {
                    conversation.screen = Screen::Setup;
                }
            }
            Some(TabBarAction::BackToChat(id)) => {
                if let Some(conversation) = conversation_mut(&mut self.tree, id) {
                    conversation.screen = Screen::Chat;
                }
            }
            Some(TabBarAction::Restart(id)) => {
                if let Some(conversation) = conversation_mut(&mut self.tree, id) {
                    conversation.start(ui.ctx(), &mut self.settings, &self.context, &self.agents);
                }
            }
            Some(TabBarAction::SendFeedback) => self.feedback_requested = true,
            Some(TabBarAction::SelfImprove) => self.self_improve_requested = true,
            None => {}
        }

        let has_conversations = self.tree.tiles.tiles().any(|tile| tile.is_pane());
        if add_requested || !has_conversations || new_conversation_shortcut_pressed(ui, panel_rect)
        {
            self.new_conversation();
        }
    }

    fn active_conversation(&self) -> Option<&Conversation> {
        let tile_id = active_tile_id(&self.tree)?;
        self.tree.tiles.get_pane(&tile_id)
    }
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

fn new_tree(first: Conversation) -> egui_tiles::Tree<Conversation> {
    egui_tiles::Tree::new_tabs(egui::Id::unique("agent_conversations"), vec![first])
}

fn conversation_mut(
    tree: &mut egui_tiles::Tree<Conversation>,
    id: ConversationId,
) -> Option<&mut Conversation> {
    conversations_mut(tree).find(|conversation| conversation.id == id)
}

fn conversations_mut(
    tree: &mut egui_tiles::Tree<Conversation>,
) -> impl Iterator<Item = &mut Conversation> {
    tree.tiles.tiles_mut().filter_map(|tile| match tile {
        Tile::Pane(conversation) => Some(conversation),
        Tile::Container(_) => None,
    })
}

/// The pane shown in the root tab bar, or the first one if none is marked active.
fn active_tile_id(tree: &egui_tiles::Tree<Conversation>) -> Option<TileId> {
    let root = tree.root()?;
    match tree.tiles.get(root)? {
        Tile::Pane(_) => Some(root),
        Tile::Container(Container::Tabs(tabs)) => {
            tabs.active.or_else(|| tabs.children.first().copied())
        }
        Tile::Container(_) => None,
    }
}

fn active_conversation_mut(tree: &mut egui_tiles::Tree<Conversation>) -> Option<&mut Conversation> {
    let tile_id = active_tile_id(tree)?;
    match tree.tiles.get_mut(tile_id)? {
        Tile::Pane(conversation) => Some(conversation),
        Tile::Container(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that prompts from the host join the busy session's queue before later chat prompts.
    #[test]
    fn host_prompts_join_busy_session_queue() {
        let mut panel = AgentPanel::with_agents(AgentSettings::default(), Vec::new());
        let session = panel.session_mut().unwrap();
        session.handle_event(re_agent::AgentEvent::SessionStarted {
            session_id: re_agent::acp::schema::v1::SessionId::new("test-session"),
            modes: None,
            config_options: Vec::new(),
        });
        session.begin_test_turn();

        panel.queue_prompt("Prompt from command palette".to_owned());
        panel.poll_events();
        let session = panel.session_mut().unwrap();
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
        let mut panel = AgentPanel::with_agents(AgentSettings::default(), Vec::new());
        let first = active_tile_id(&panel.tree).unwrap();
        panel.queue_prompt("First prompt".to_owned());
        panel.queue_prompt("Second prompt".to_owned());
        panel.queue_prompt("   ".to_owned());
        panel.poll_events();
        panel.open_conversation(SessionContext::default(), "Other conversation");
        panel.poll_events();

        let Some(Tile::Pane(conversation)) = panel.tree.tiles.get(first) else {
            panic!("first conversation should still exist");
        };
        assert_eq!(conversation.screen, Screen::Setup);
        assert_eq!(
            conversation
                .pending_prompts
                .iter()
                .map(|prompt| prompt.text.as_str())
                .collect::<Vec<_>>(),
            ["First prompt", "Second prompt"]
        );
        assert_eq!(
            panel
                .active_conversation()
                .unwrap()
                .pending_prompts
                .iter()
                .map(|prompt| prompt.text.as_str())
                .collect::<Vec<_>>(),
            ["Other conversation"],
        );
    }
}
