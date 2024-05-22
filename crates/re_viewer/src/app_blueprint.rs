use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{DataRow, EntityPath, RowId, TimePoint};
use re_types::blueprint::components::PanelState;
use re_viewer_context::{CommandSender, StoreContext, SystemCommand, SystemCommandSender};

pub const TOP_PANEL_PATH: &str = "top_panel";
pub const BLUEPRINT_PANEL_PATH: &str = "blueprint_panel";
pub const SELECTION_PANEL_PATH: &str = "selection_panel";
pub const TIME_PANEL_PATH: &str = "time_panel";

/// Blueprint for top-level application
pub struct AppBlueprint<'a> {
    store_ctx: Option<&'a StoreContext<'a>>,
    is_narrow_screen: bool,
    pub top_panel_state: PanelState,
    pub blueprint_panel_state: PanelState,
    pub selection_panel_state: PanelState,
    pub time_panel_state: PanelState,
}

impl<'a> AppBlueprint<'a> {
    pub fn new(
        store_ctx: Option<&'a StoreContext<'_>>,
        query: &LatestAtQuery,
        egui_ctx: &egui::Context,
    ) -> Self {
        let blueprint_db = store_ctx.map(|ctx| ctx.blueprint);
        let screen_size = egui_ctx.screen_rect().size();
        let mut ret = Self {
            store_ctx,
            is_narrow_screen: screen_size.x < 600.0,
            top_panel_state: PanelState::Expanded,
            blueprint_panel_state: if screen_size.x > 750.0 {
                PanelState::Expanded
            } else {
                PanelState::Collapsed
            },
            selection_panel_state: if screen_size.x > 1000.0 {
                PanelState::Expanded
            } else {
                PanelState::Collapsed
            },
            time_panel_state: if screen_size.y > 600.0 {
                PanelState::Expanded
            } else {
                PanelState::Collapsed
            },
        };

        if let Some(blueprint_db) = blueprint_db {
            if let Some(state) = load_panel_state(&TOP_PANEL_PATH.into(), blueprint_db, query) {
                ret.top_panel_state = state;
            }
            if let Some(state) = load_panel_state(&BLUEPRINT_PANEL_PATH.into(), blueprint_db, query)
            {
                ret.blueprint_panel_state = state;
            }
            if let Some(state) = load_panel_state(&SELECTION_PANEL_PATH.into(), blueprint_db, query)
            {
                ret.selection_panel_state = state;
            }
            if let Some(state) = load_panel_state(&TIME_PANEL_PATH.into(), blueprint_db, query) {
                ret.time_panel_state = state;
            }
        }

        ret
    }

    pub fn toggle_blueprint_panel(&self, command_sender: &CommandSender) {
        let new_state = self.blueprint_panel_state.toggle();
        self.send_panel_state(BLUEPRINT_PANEL_PATH, new_state, command_sender);

        // Toggle the opposite side if this panel is visible to save on screen real estate
        if self.is_narrow_screen && new_state.is_expanded() {
            self.send_panel_state(SELECTION_PANEL_PATH, PanelState::Hidden, command_sender);
        }
    }

    pub fn toggle_selection_panel(&self, command_sender: &CommandSender) {
        let new_state = self.selection_panel_state.toggle();
        self.send_panel_state(SELECTION_PANEL_PATH, new_state, command_sender);

        // Toggle the opposite side if this panel is visible to save on screen real estate
        if self.is_narrow_screen && new_state.is_expanded() {
            self.send_panel_state(BLUEPRINT_PANEL_PATH, PanelState::Hidden, command_sender);
        }
    }

    pub fn toggle_time_panel(&self, command_sender: &CommandSender) {
        self.send_panel_state(
            TIME_PANEL_PATH,
            self.time_panel_state.toggle(),
            command_sender,
        );
    }
}

pub fn setup_welcome_screen_blueprint(welcome_screen_blueprint: &mut EntityDb) {
    for (panel_name, value) in [
        (TOP_PANEL_PATH, PanelState::Expanded),
        (BLUEPRINT_PANEL_PATH, PanelState::Expanded),
        (SELECTION_PANEL_PATH, PanelState::Hidden),
        (TIME_PANEL_PATH, PanelState::Hidden),
    ] {
        let entity_path = EntityPath::from(panel_name);
        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::default();

        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, [value]).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

        welcome_screen_blueprint.add_data_row(row).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't
    }
}

// ----------------------------------------------------------------------------

impl<'a> AppBlueprint<'a> {
    fn send_panel_state(
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
