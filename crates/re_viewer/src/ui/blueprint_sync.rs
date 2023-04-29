use re_log_types::{parse_entity_path, DataRow, RowId, TimePoint};

use crate::blueprint_components::PanelState;

use super::Blueprint;

// Resolving and applying updates
impl Blueprint {
    pub fn process_updates(&self, snapshot: &Self, blueprint_db: &mut re_data_store::LogDb) {
        if self.blueprint_panel_expanded != snapshot.blueprint_panel_expanded {
            set_panel_expanded(
                blueprint_db,
                PanelState::BLUEPRINT_PANEL,
                self.blueprint_panel_expanded,
            );
        }
        if self.selection_panel_expanded != snapshot.selection_panel_expanded {
            set_panel_expanded(
                blueprint_db,
                PanelState::SELECTION_PANEL,
                self.selection_panel_expanded,
            );
        }
        if self.time_panel_expanded != snapshot.time_panel_expanded {
            set_panel_expanded(
                blueprint_db,
                PanelState::TIMELINE_PANEL,
                self.time_panel_expanded,
            );
        }
    }
}

pub fn set_panel_expanded(
    blueprint_db: &mut re_data_store::LogDb,
    panel_name: &str,
    expanded: bool,
) {
    // TODO(jleibs): NO UNWRAP
    let entity_path = parse_entity_path(panel_name).unwrap();
    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let panel_state = PanelState { expanded };

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        timepoint,
        1,
        [panel_state].as_slice(),
    );

    // TODO(jleibs) Is this safe? Get rid of unwrap
    blueprint_db.entity_db.try_add_data_row(&row).unwrap();
}
