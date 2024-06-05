use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{DataRow, EntityPath, RowId};
use re_types::blueprint::components::PanelState;
use re_viewer_context::{CommandSender, StoreContext, SystemCommand, SystemCommandSender};

const TOP_PANEL_PATH: &str = "top_panel";
const BLUEPRINT_PANEL_PATH: &str = "blueprint_panel";
const SELECTION_PANEL_PATH: &str = "selection_panel";
const TIME_PANEL_PATH: &str = "time_panel";

/// Blueprint for top-level application
pub struct AppBlueprint<'a> {
    store_ctx: Option<&'a StoreContext<'a>>,
    is_narrow_screen: bool,
    panel_states: PanelStates,
    overrides: Option<PanelStateOverrides>,
}

#[derive(Debug, Clone, Copy)]
pub struct PanelStates {
    pub top: PanelState,
    pub blueprint: PanelState,
    pub selection: PanelState,
    pub time: PanelState,
}

impl<'a> AppBlueprint<'a> {
    pub fn new(
        store_ctx: Option<&'a StoreContext<'_>>,
        query: &LatestAtQuery,
        egui_ctx: &egui::Context,
        overrides: Option<PanelStateOverrides>,
    ) -> Self {
        let blueprint_db = store_ctx.map(|ctx| ctx.blueprint);
        let screen_size = egui_ctx.screen_rect().size();
        let mut ret = Self {
            store_ctx,
            is_narrow_screen: screen_size.x < 600.0,
            panel_states: PanelStates {
                top: PanelState::Expanded,
                blueprint: if screen_size.x > 750.0 {
                    PanelState::Expanded
                } else {
                    PanelState::Collapsed
                },
                selection: if screen_size.x > 1000.0 {
                    PanelState::Expanded
                } else {
                    PanelState::Collapsed
                },
                time: if screen_size.y > 600.0 {
                    PanelState::Expanded
                } else {
                    PanelState::Collapsed
                },
            },
            overrides,
        };

        if let Some(blueprint_db) = blueprint_db {
            if let Some(state) = load_panel_state(&TOP_PANEL_PATH.into(), blueprint_db, query) {
                ret.panel_states.top = state;
            }
            if let Some(state) = load_panel_state(&BLUEPRINT_PANEL_PATH.into(), blueprint_db, query)
            {
                ret.panel_states.blueprint = state;
            }
            if let Some(state) = load_panel_state(&SELECTION_PANEL_PATH.into(), blueprint_db, query)
            {
                ret.panel_states.selection = state;
            }
            if let Some(state) = load_panel_state(&TIME_PANEL_PATH.into(), blueprint_db, query) {
                ret.panel_states.time = state;
            }
        }

        ret
    }

    pub fn top_panel_state(&self) -> PanelState {
        self.overrides
            .and_then(|o| o.top)
            .unwrap_or(self.panel_states.top)
    }

    pub fn blueprint_panel_state(&self) -> PanelState {
        self.overrides
            .and_then(|o| o.blueprint)
            .unwrap_or(self.panel_states.blueprint)
    }

    pub fn selection_panel_state(&self) -> PanelState {
        self.overrides
            .and_then(|o| o.selection)
            .unwrap_or(self.panel_states.selection)
    }

    pub fn time_panel_state(&self) -> PanelState {
        self.overrides
            .and_then(|o| o.time)
            .unwrap_or(self.panel_states.time)
    }

    pub fn toggle_top_panel(&self, command_sender: &CommandSender) {
        // don't toggle if it is overridden
        if self.overrides.is_some_and(|o| o.top.is_some()) {
            return;
        }

        self.send_panel_state(
            TOP_PANEL_PATH,
            self.panel_states.top.toggle(),
            command_sender,
        );
    }

    pub fn toggle_blueprint_panel(&self, command_sender: &CommandSender) {
        // don't toggle if it is overridden
        if self.overrides.is_some_and(|o| o.blueprint.is_some()) {
            return;
        }

        let new_state = self.panel_states.blueprint.toggle();
        self.send_panel_state(BLUEPRINT_PANEL_PATH, new_state, command_sender);

        // Toggle the opposite side if this panel is visible to save on screen real estate
        if self.is_narrow_screen && new_state.is_expanded() {
            self.send_panel_state(SELECTION_PANEL_PATH, PanelState::Hidden, command_sender);
        }
    }

    pub fn toggle_selection_panel(&self, command_sender: &CommandSender) {
        // don't toggle if it is overridden
        if self.overrides.is_some_and(|o| o.selection.is_some()) {
            return;
        }

        let new_state = self.panel_states.selection.toggle();
        self.send_panel_state(SELECTION_PANEL_PATH, new_state, command_sender);

        // Toggle the opposite side if this panel is visible to save on screen real estate
        if self.is_narrow_screen && new_state.is_expanded() {
            self.send_panel_state(BLUEPRINT_PANEL_PATH, PanelState::Hidden, command_sender);
        }
    }

    pub fn toggle_time_panel(&self, command_sender: &CommandSender) {
        // don't toggle if it is overridden
        if self.overrides.is_some_and(|o| o.time.is_some()) {
            return;
        }

        self.send_panel_state(
            TIME_PANEL_PATH,
            self.panel_states.time.toggle(),
            command_sender,
        );
    }

    pub fn blueprint_panel_overridden(&self) -> bool {
        self.overrides.is_some_and(|s| s.blueprint.is_some())
    }

    pub fn selection_panel_overridden(&self) -> bool {
        self.overrides.is_some_and(|s| s.selection.is_some())
    }

    pub fn time_panel_overridden(&self) -> bool {
        self.overrides.is_some_and(|s| s.time.is_some())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PanelStateOverrides {
    pub top: Option<PanelState>,
    pub blueprint: Option<PanelState>,
    pub selection: Option<PanelState>,
    pub time: Option<PanelState>,
}

pub fn setup_welcome_screen_blueprint(welcome_screen_blueprint: &mut EntityDb) {
    // Most things are hidden in the welcome screen:
    for (panel_name, value) in [
        (TOP_PANEL_PATH, PanelState::Expanded),
        (BLUEPRINT_PANEL_PATH, PanelState::Hidden),
        (SELECTION_PANEL_PATH, PanelState::Hidden),
        (TIME_PANEL_PATH, PanelState::Hidden),
    ] {
        let entity_path = EntityPath::from(panel_name);

        let timepoint = re_viewer_context::blueprint_timepoint_for_writes(welcome_screen_blueprint);

        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, [value]).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

        welcome_screen_blueprint.add_data_row(row).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't
    }
}

// ----------------------------------------------------------------------------

impl<'a> AppBlueprint<'a> {
    pub(crate) fn send_panel_state(
        &self,
        panel_name: &str,
        value: PanelState,
        command_sender: &CommandSender,
    ) {
        if let Some(store_ctx) = self.store_ctx {
            let entity_path = EntityPath::from(panel_name);

            let timepoint = store_ctx.blueprint_timepoint_for_writes();

            let row =
                DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, [value]).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

            command_sender.send_system(SystemCommand::UpdateBlueprint(
                store_ctx.blueprint.store_id().clone(),
                vec![row],
            ));
        }
    }
}

fn load_panel_state(
    path: &EntityPath,
    blueprint_db: &re_entity_db::EntityDb,
    query: &LatestAtQuery,
) -> Option<PanelState> {
    re_tracing::profile_function!();
    // TODO(#5607): what should happen if the promise is still pending?
    blueprint_db
        .latest_at_component_quiet::<PanelState>(path, query)
        .map(|p| p.value)
}
