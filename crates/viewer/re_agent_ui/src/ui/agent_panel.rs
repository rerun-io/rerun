//! The complete agent panel: one tab per conversation, each showing setup until its agent runs,
//! then the chat.

use egui_tiles::{Container, Tile, TileId, Tiles};
use re_agent::acp::schema::v1::{SessionModeId, SessionModeState};
use re_ui::{UiExt as _, icons};

use super::Screen;
use super::chat_ui::{ChatInput, chat_ui};
use super::setup_ui::setup_ui;
use re_agent::AgentEntry;
use re_agent::{AgentSession, Phase};
use re_agent::{AgentSettings, SessionContext};
use re_agent::{Transcript, TranscriptItem};

/// Longest tab title before it is cut with an ellipsis.
const MAX_TAB_TITLE_CHARS: usize = 24;

/// One chat with one agent: the content of a tab.
struct Conversation {
    session: AgentSession,
    input: ChatInput,

    /// [`Screen::Chat`] with an idle session means "start the agent on the next frame".
    screen: Screen,
}

impl Conversation {
    /// A fresh conversation. Skips the setup screen when the user has completed it before.
    fn new(settings: &AgentSettings, auto_approve: bool) -> Self {
        let mut session = AgentSession::default();
        session.set_auto_approve(auto_approve);
        Self {
            session,
            input: ChatInput::default(),
            screen: if settings.setup_done {
                Screen::Chat
            } else {
                Screen::Setup
            },
        }
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        settings: &mut AgentSettings,
        context: &SessionContext,
        agents: &[AgentEntry],
    ) {
        match settings.launch_config(agents, context) {
            Ok(config) => {
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
                    TranscriptItem::User { text } => {
                        Some(text.lines().next().unwrap_or_default().to_owned())
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
            .find(|agent| agent.profile.id == settings.profile_id)
            .map(|agent| agent.profile.login_hint.as_str())
            .filter(|hint| !hint.is_empty());
        chat_ui(
            ui,
            &mut self.session,
            &mut self.input,
            settings.show_thoughts,
            login_hint,
        );
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

/// What the buttons at the right end of the tab bar asked for.
#[derive(Clone, Copy)]
enum TabAction {
    OpenSetup,
    BackToChat,
    Restart,
}

/// Tabs only: no dragging tabs into splits, closable tabs, "+" for a new conversation,
/// and the active conversation's setup/restart buttons at the right end of the bar.
struct TabsBehavior<'a> {
    settings: &'a mut AgentSettings,
    context: &'a SessionContext,
    agents: &'a mut Vec<AgentEntry>,
    add_requested: bool,
    tab_action: Option<(TileId, TabAction)>,
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
        conversation.title(self.settings, self.agents).into()
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
        if ui
            .small_icon_button(&icons::ADD, "New conversation")
            .on_hover_text("New conversation")
            .clicked()
        {
            self.add_requested = true;
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

        ui.add_space(8.0);
        if conversation.screen == Screen::Setup {
            if has_agent
                && ui
                    .small_icon_button(&icons::CLOSE, "Back to the conversation")
                    .on_hover_text("Back to the conversation")
                    .clicked()
            {
                self.tab_action = Some((active, TabAction::BackToChat));
            }
        } else {
            if ui
                .small_icon_button(&icons::SETTINGS, "Agent setup")
                .on_hover_text("Agent setup")
                .clicked()
            {
                self.tab_action = Some((active, TabAction::OpenSetup));
            }
            if ui
                .small_icon_button(&icons::RESET, "New session (restarts the agent)")
                .on_hover_text("New session (restarts the agent)")
                .clicked()
            {
                self.tab_action = Some((active, TabAction::Restart));
            }
        }
    }

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        re_ui::design_tokens_of_visuals(visuals).tab_bar_color
    }

    fn tab_bar_height(&self, style: &egui::Style) -> f32 {
        re_ui::design_tokens_of_visuals(&style.visuals).title_bar_height()
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

/// The complete agent UI: a tab per conversation. Owns the sessions and the shared settings.
pub struct AgentPanel {
    settings: AgentSettings,
    context: SessionContext,
    agents: Vec<AgentEntry>,
    tree: egui_tiles::Tree<Conversation>,
    auto_approve: bool,
}

impl AgentPanel {
    pub fn new(settings: AgentSettings) -> Self {
        Self::with_agents(settings, AgentEntry::detect_all())
    }

    /// Like [`Self::new`], but with an explicit list of agents instead of probing the machine.
    ///
    /// On a first run with at least one installed agent, one is picked and started right away;
    /// the setup screen only appears when nothing is installed.
    pub fn with_agents(mut settings: AgentSettings, agents: Vec<AgentEntry>) -> Self {
        if settings.profile_id.is_empty() {
            if let Some(agent) = AgentEntry::pick_default(&agents) {
                settings.profile_id = agent.profile.id.clone();
                settings.setup_done = true;
            } else if let Some(agent) = agents.first() {
                settings.profile_id = agent.profile.id.clone();
            }
        }

        let first = Conversation::new(&settings, false);
        Self {
            settings,
            context: SessionContext::default(),
            agents,
            tree: new_tree(first),
            auto_approve: false,
        }
    }

    /// What every new session is told about the host. Applies to sessions started from now on.
    pub fn set_context(&mut self, context: SessionContext) {
        self.context = context;
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
        if let Some(session) = self.session_mut() {
            session.set_mode(mode_id);
        }
    }

    /// Sends a prompt in the active tab as if the user had typed it.
    /// Returns `false` if it was dropped because the agent is not [`Self::is_ready`].
    pub fn send_prompt(&mut self, text: impl Into<String>) -> bool {
        self.session_mut()
            .is_some_and(|session| session.send_prompt(text))
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
        let conversation = Conversation::new(&self.settings, self.auto_approve);

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

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        for conversation in conversations_mut(&mut self.tree) {
            conversation.session.poll_events();
        }

        let mut behavior = TabsBehavior {
            settings: &mut self.settings,
            context: &self.context,
            agents: &mut self.agents,
            add_requested: false,
            tab_action: None,
        };
        self.tree.ui(&mut behavior, ui);
        let TabsBehavior {
            add_requested,
            tab_action,
            ..
        } = behavior;

        if let Some((tile_id, action)) = tab_action
            && let Some(Tile::Pane(conversation)) = self.tree.tiles.get_mut(tile_id)
        {
            match action {
                TabAction::OpenSetup => conversation.screen = Screen::Setup,
                TabAction::BackToChat => conversation.screen = Screen::Chat,
                TabAction::Restart => {
                    conversation.start(ui.ctx(), &mut self.settings, &self.context, &self.agents);
                }
            }
        }

        let has_conversations = self.tree.tiles.tiles().any(|tile| tile.is_pane());
        if add_requested || !has_conversations {
            self.new_conversation();
        }
    }

    fn active_conversation(&self) -> Option<&Conversation> {
        let tile_id = active_tile_id(&self.tree)?;
        self.tree.tiles.get_pane(&tile_id)
    }
}

fn new_tree(first: Conversation) -> egui_tiles::Tree<Conversation> {
    egui_tiles::Tree::new_tabs("agent_conversations", vec![first])
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
