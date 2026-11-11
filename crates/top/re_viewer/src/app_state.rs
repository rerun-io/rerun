use re_redap_browser::RedapServers;
use re_viewer_context::AppOptions;

/// The viewer state that is persisted between runs.
///
/// Only put state here that should survive a restart of the viewer.
/// Everything else belongs in `AppCore`.
///
/// The panels live here too, so that each one can persist whatever it wants to.
/// Panel state that should not be persisted is marked `#[serde(skip)]`.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Settings shared by all conversations in the agent panel.
    ///
    /// This stays in `re_viewer` so `re_viewer_context` does not depend on the agent UI.
    pub(crate) agent_settings: re_agent::AgentSettings,

    /// Whether the agent panel is open. Persisted, so it comes back on the next run.
    pub(crate) agent_panel_open: bool,

    /// Set once the agent panel has been open, however it was opened.
    pub(crate) agent_panel_ever_opened: bool,

    pub(crate) selection_panel: re_selection_panel::SelectionPanel,
    pub(crate) time_panel: re_time_panel::TimePanel,
    pub(crate) blueprint_time_panel: re_time_panel::TimePanel,
    #[serde(skip)]
    pub(crate) blueprint_tree: re_blueprint_tree::BlueprintTree,
    #[serde(skip)]
    pub(crate) recording_panel: re_recording_panel::RecordingPanel,

    #[serde(skip)]
    pub(crate) welcome_screen: crate::ui::WelcomeScreen,

    #[serde(skip)]
    pub(crate) datastore_ui: re_chunk_store_ui::DatastoreUi,

    /// Redap server catalogs and browser UI
    pub(crate) redap_servers: RedapServers,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_options: Default::default(),
            agent_settings: Default::default(),
            agent_panel_open: false,
            agent_panel_ever_opened: false,
            selection_panel: Default::default(),
            time_panel: Default::default(),
            blueprint_time_panel: re_time_panel::TimePanel::new_blueprint_panel(),
            recording_panel: Default::default(),
            blueprint_tree: Default::default(),
            welcome_screen: Default::default(),
            datastore_ui: Default::default(),
            redap_servers: Default::default(),
        }
    }
}

impl AppState {
    pub fn set_examples_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        self.welcome_screen.set_examples_manifest_url(egui_ctx, url);
    }

    pub fn app_options(&self) -> &AppOptions {
        &self.app_options
    }

    pub fn app_options_mut(&mut self) -> &mut AppOptions {
        &mut self.app_options
    }
}
