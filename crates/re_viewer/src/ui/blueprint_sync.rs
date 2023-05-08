use arrow2_convert::field::ArrowField;
use re_log_types::{parse_entity_path, Component, DataCell, DataRow, RowId, TimePoint};

use crate::blueprint_components::{PanelState, SpaceViewComponent, ViewportComponent};

use super::{Blueprint, SpaceView, Viewport};

// Resolving and applying updates
impl Blueprint {
    pub fn process_updates(&self, snapshot: &Self, blueprint_db: &mut re_data_store::LogDb) {
        // Update the panel states
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
        // Save the viewport state
        if self.viewport != snapshot.viewport {
            re_log::debug!("Viewport change detected. Saving modifications.");
            store_viewport(blueprint_db, &self.viewport);

            // Since space views are part of the viewport, we only need to handle them here

            // Add any new or modified space views
            for id in self.viewport.space_view_ids() {
                let space_view = self.viewport.space_view(id).unwrap();
                if let Some(snapshot_space_view) = snapshot.viewport.space_view(id) {
                    if space_view == snapshot_space_view {
                        continue;
                    }
                }
                store_space_view(blueprint_db, space_view);
            }

            // Remove any deleted space views
            for id in snapshot.viewport.space_view_ids() {
                let space_view = snapshot.viewport.space_view(id).unwrap();
                if self.viewport.space_view(id).is_none() {
                    clear_space_view(blueprint_db, space_view);
                }
            }
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

    let mut row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        timepoint,
        1,
        [panel_state].as_slice(),
    );
    row.compute_all_size_bytes();

    // TODO(jleibs) Is this safe? Get rid of unwrap
    blueprint_db.entity_db.try_add_data_row(&row).unwrap();
}

pub fn store_space_view(blueprint_db: &mut re_data_store::LogDb, space_view: &SpaceView) {
    // TODO(jleibs): NO UNWRAP
    let entity_path = parse_entity_path(
        format!("{}/{}", SpaceViewComponent::SPACEVIEW_PREFIX, space_view.id).as_str(),
    )
    .unwrap();
    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let component = SpaceViewComponent {
        space_view: space_view.clone(),
    };

    let mut row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        timepoint,
        1,
        [component].as_slice(),
    );
    row.compute_all_size_bytes();

    // TODO(jleibs) Is this safe? Get rid of unwrap
    blueprint_db.entity_db.try_add_data_row(&row).unwrap();
}

pub fn clear_space_view(blueprint_db: &mut re_data_store::LogDb, space_view: &SpaceView) {
    // TODO(jleibs): NO UNWRAP
    let entity_path = parse_entity_path(
        format!("{}/{}", SpaceViewComponent::SPACEVIEW_PREFIX, space_view.id).as_str(),
    )
    .unwrap();
    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let cell =
        DataCell::from_arrow_empty(SpaceViewComponent::name(), SpaceViewComponent::data_type());

    let mut row = DataRow::from_cells1(RowId::random(), entity_path, timepoint, 0, cell);
    row.compute_all_size_bytes();

    // TODO(jleibs) Is this safe? Get rid of unwrap
    blueprint_db.entity_db.try_add_data_row(&row).unwrap();
}

pub fn store_viewport(blueprint_db: &mut re_data_store::LogDb, viewport: &Viewport) {
    // TODO(jleibs): NO UNWRAP
    let entity_path = parse_entity_path(ViewportComponent::VIEWPORT).unwrap();
    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let component = ViewportComponent {
        space_view_keys: viewport.space_views.keys().cloned().collect(),
        visible: viewport.visible.clone(),
        trees: viewport.trees.clone(),
        maximized: viewport.maximized,
        has_been_user_edited: viewport.has_been_user_edited,
    };

    let mut row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        timepoint,
        1,
        [component].as_slice(),
    );
    row.compute_all_size_bytes();

    // TODO(jleibs) Is this safe? Get rid of unwrap
    blueprint_db.entity_db.try_add_data_row(&row).unwrap();
}
