use re_data_store::StoreDb;
use re_log_types::{DataRow, EntityPath, RowId, TimePoint};
use re_types::blueprint::PanelView;
use re_viewer_context::{CommandSender, StoreContext, SystemCommand, SystemCommandSender};

/// Blueprint for top-level application
pub struct AppBlueprint<'a> {
    blueprint_db: Option<&'a StoreDb>,
    is_narrow_screen: bool,
    pub blueprint_panel_expanded: bool,
    pub selection_panel_expanded: bool,
    pub time_panel_expanded: bool,
}

impl<'a> AppBlueprint<'a> {
    pub fn new(store_ctx: Option<&'a StoreContext<'_>>, egui_ctx: &egui::Context) -> Self {
        let blueprint_db = store_ctx.map(|ctx| ctx.blueprint);
        let screen_size = egui_ctx.screen_rect().size();
        let mut ret = Self {
            blueprint_db,
            is_narrow_screen: screen_size.x < 600.0,
            blueprint_panel_expanded: screen_size.x > 750.0,
            selection_panel_expanded: screen_size.x > 1000.0,
            time_panel_expanded: screen_size.y > 600.0,
        };

        if let Some(blueprint_db) = blueprint_db {
            if let Some(expanded) =
                load_panel_state(&PanelView::BLUEPRINT_VIEW_PATH.into(), blueprint_db)
            {
                ret.blueprint_panel_expanded = expanded;
            }
            if let Some(expanded) =
                load_panel_state(&PanelView::SELECTION_VIEW_PATH.into(), blueprint_db)
            {
                ret.selection_panel_expanded = expanded;
            }
            if let Some(expanded) =
                load_panel_state(&PanelView::TIMELINE_VIEW_PATH.into(), blueprint_db)
            {
                ret.time_panel_expanded = expanded;
            }
        }

        ret
    }

    pub fn toggle_blueprint_panel(&self, command_sender: &CommandSender) {
        let blueprint_panel_expanded = !self.blueprint_panel_expanded;
        self.send_panel_expanded(
            PanelView::BLUEPRINT_VIEW_PATH,
            blueprint_panel_expanded,
            command_sender,
        );
        if self.is_narrow_screen && self.blueprint_panel_expanded {
            self.send_panel_expanded(PanelView::SELECTION_VIEW_PATH, false, command_sender);
        }
    }

    pub fn toggle_selection_panel(&self, command_sender: &CommandSender) {
        let selection_panel_expanded = !self.selection_panel_expanded;
        self.send_panel_expanded(
            PanelView::SELECTION_VIEW_PATH,
            selection_panel_expanded,
            command_sender,
        );
        if self.is_narrow_screen && self.blueprint_panel_expanded {
            self.send_panel_expanded(PanelView::BLUEPRINT_VIEW_PATH, false, command_sender);
        }
    }

    pub fn toggle_time_panel(&self, command_sender: &CommandSender) {
        self.send_panel_expanded(
            PanelView::TIMELINE_VIEW_PATH,
            !self.time_panel_expanded,
            command_sender,
        );
    }
}

pub fn setup_welcome_screen_blueprint(welcome_screen_blueprint: &mut StoreDb) {
    for (panel_name, is_expanded) in [
        (PanelView::BLUEPRINT_VIEW_PATH, true),
        (PanelView::SELECTION_VIEW_PATH, false),
        (PanelView::TIMELINE_VIEW_PATH, false),
    ] {
        let entity_path = EntityPath::from(panel_name);
        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        let component = PanelView { is_expanded };

        let row =
            DataRow::from_cells1_sized(RowId::random(), entity_path, timepoint, 1, [component])
                .unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

        welcome_screen_blueprint.add_data_row(&row).unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't
    }
}

// ----------------------------------------------------------------------------

impl<'a> AppBlueprint<'a> {
    fn send_panel_expanded(
        &self,
        panel_name: &str,
        is_expanded: bool,
        command_sender: &CommandSender,
    ) {
        if let Some(blueprint_db) = self.blueprint_db {
            let entity_path = EntityPath::from(panel_name);
            // TODO(jleibs): Seq instead of timeless?
            let timepoint = TimePoint::timeless();

            let component = PanelView { is_expanded };

            let row =
                DataRow::from_cells1_sized(RowId::random(), entity_path, timepoint, 1, [component])
                    .unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

            command_sender.send_system(SystemCommand::UpdateBlueprint(
                blueprint_db.store_id().clone(),
                vec![row],
            ));
        }
    }
}

fn load_panel_state(path: &EntityPath, blueprint_db: &re_data_store::StoreDb) -> Option<bool> {
    re_tracing::profile_function!();
    blueprint_db
        .store()
        .query_timeless_component::<PanelView>(path)
        .map(|p| p.is_expanded)
}
