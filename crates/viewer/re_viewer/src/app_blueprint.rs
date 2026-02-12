use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimePoint};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_sdk_types::AsComponents;
use re_sdk_types::blueprint::archetypes::{PanelBlueprint, TimePanelBlueprint};
use re_sdk_types::blueprint::components::PanelState;
use re_viewer_context::{
    CommandSender, SystemCommand, SystemCommandSender as _, TIME_PANEL_PATH,
    blueprint_timepoint_for_writes,
};

const TOP_PANEL_PATH: &str = "top_panel";
const BLUEPRINT_PANEL_PATH: &str = "blueprint_panel";
const SELECTION_PANEL_PATH: &str = "selection_panel";

/// Blueprint for top-level application
pub struct AppBlueprint<'a> {
    blueprint_db: Option<&'a EntityDb>,
    is_narrow_screen: bool,
    panel_states: PanelStates,
    overrides: Option<PanelStateOverrides>,
}

#[derive(Debug, Clone)]
pub struct PanelStates {
    pub top: PanelState,
    pub blueprint: PanelState,
    pub selection: PanelState,
    pub time: PanelState,
}

impl<'a> AppBlueprint<'a> {
    pub fn new(
        blueprint_db: Option<&'a EntityDb>,
        query: &LatestAtQuery,
        egui_ctx: &egui::Context,
        overrides: Option<PanelStateOverrides>,
    ) -> Self {
        let screen_size = egui_ctx.content_rect().size();
        let mut ret = Self {
            blueprint_db,
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
            re_log::debug_assert_eq!(
                blueprint_db.store_kind(),
                re_log_types::StoreKind::Blueprint,
                "the entity db backing an app blueprint has to be a blueprint store."
            );

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
        (BLUEPRINT_PANEL_PATH, PanelState::Expanded), // We always want to show this to orient the user
        (SELECTION_PANEL_PATH, PanelState::Hidden),   // There is nothing in here in welcome screen
        (TIME_PANEL_PATH, PanelState::Hidden),        // There is nothing in here in welcome screen
    ] {
        let timepoint = re_viewer_context::blueprint_timepoint_for_writes(welcome_screen_blueprint);

        let chunk = get_panel_state_chunk(panel_name, timepoint, value);

        welcome_screen_blueprint
            .add_chunk(&Arc::new(chunk))
            .expect("Failed to add new chunk for welcome screen");
    }
}

// ----------------------------------------------------------------------------

impl AppBlueprint<'_> {
    pub(crate) fn send_panel_state(
        &self,
        panel_name: &str,
        value: PanelState,
        command_sender: &CommandSender,
    ) {
        if let Some(blueprint_db) = self.blueprint_db {
            let timepoint = blueprint_timepoint_for_writes(blueprint_db);

            let chunk = get_panel_state_chunk(panel_name, timepoint, value);

            command_sender.send_system(SystemCommand::AppendToStore(
                blueprint_db.store_id().clone(),
                vec![chunk],
            ));
        }
    }
}

fn get_panel_state_chunk(panel_name: &str, timepoint: TimePoint, value: PanelState) -> Chunk {
    let entity_path = EntityPath::from(panel_name);

    let component_update: &dyn AsComponents = if panel_name == TIME_PANEL_PATH {
        &TimePanelBlueprint::update_fields().with_state(value)
    } else {
        &PanelBlueprint::update_fields().with_state(value)
    };

    Chunk::builder(entity_path)
        .with_archetype(RowId::new(), timepoint, component_update)
        .build()
        // All builtin types, no reason for this to ever fail.
        .expect("Failed to build chunk.")
}

fn load_panel_state(
    path: &EntityPath,
    blueprint_db: &re_entity_db::EntityDb,
    query: &LatestAtQuery,
) -> Option<PanelState> {
    re_tracing::profile_function!();
    let component = if path == &TIME_PANEL_PATH.into() {
        TimePanelBlueprint::descriptor_state().component
    } else {
        PanelBlueprint::descriptor_state().component
    };

    blueprint_db
        .latest_at_component_quiet::<PanelState>(path, query, component)
        .map(|(_index, p)| p)
}
