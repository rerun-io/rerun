use arrow2_convert::field::ArrowField;
use re_data_store::LogDb;
use re_log_types::{
    Component, DataCell, DataRow, EntityPath, RowId, SerializableComponent, TimePoint,
};
use re_viewer_context::SpaceViewId;

use crate::blueprint_components::{
    panel::PanelState,
    space_view::SpaceViewComponent,
    viewport::{
        AutoSpaceViews, SpaceViewMaximized, SpaceViewVisibility, ViewportLayout, VIEWPORT_PATH,
    },
};

use super::{Blueprint, SpaceView, Viewport};

// TODO(jleibs): Put this somewhere more global
pub fn store_one_component<C: SerializableComponent>(
    log_db: &mut LogDb,
    entity_path: &EntityPath,
    timepoint: &TimePoint,
    component: C,
) {
    let mut row = DataRow::from_cells1(
        RowId::random(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component].as_slice(),
    );
    row.compute_all_size_bytes();

    match log_db.entity_db.try_add_data_row(&row) {
        Ok(()) => {}
        Err(err) => {
            re_log::warn_once!(
                "Failed to store component {}.{}: {err}",
                entity_path,
                C::name(),
            );
        }
    }
}

// Resolving and applying updates
impl Blueprint {
    pub fn process_updates(&self, snapshot: &Self, blueprint_db: &mut re_data_store::LogDb) {
        // Update the panel states
        sync_panel_expanded(
            blueprint_db,
            PanelState::BLUEPRINT_PANEL,
            self.blueprint_panel_expanded,
            snapshot.blueprint_panel_expanded,
        );
        sync_panel_expanded(
            blueprint_db,
            PanelState::SELECTION_PANEL,
            self.selection_panel_expanded,
            snapshot.selection_panel_expanded,
        );
        sync_panel_expanded(
            blueprint_db,
            PanelState::TIMELINE_PANEL,
            self.time_panel_expanded,
            snapshot.time_panel_expanded,
        );

        sync_viewport(blueprint_db, &self.viewport, &snapshot.viewport);

        // Add any new or modified space views
        for id in self.viewport.space_view_ids() {
            if let Some(space_view) = self.viewport.space_view(id) {
                sync_space_view(blueprint_db, space_view, snapshot.viewport.space_view(id));
            }
        }

        // Remove any deleted space views
        for space_view_id in snapshot.viewport.space_view_ids() {
            if self.viewport.space_view(space_view_id).is_none() {
                clear_space_view(blueprint_db, space_view_id);
            }
        }
    }
}

pub fn sync_panel_expanded(
    blueprint_db: &mut re_data_store::LogDb,
    panel_name: &str,
    expanded: bool,
    snapshot: bool,
) {
    if expanded != snapshot {
        let entity_path = EntityPath::from(panel_name);
        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        let component = PanelState { expanded };

        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }
}

pub fn sync_space_view(
    blueprint_db: &mut re_data_store::LogDb,
    space_view: &SpaceView,
    snapshot: Option<&SpaceView>,
) {
    if Some(space_view) != snapshot {
        let entity_path = EntityPath::from(format!(
            "{}/{}",
            SpaceViewComponent::SPACEVIEW_PREFIX,
            space_view.id
        ));

        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        let component = SpaceViewComponent {
            space_view: space_view.clone(),
        };

        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }
}

pub fn clear_space_view(blueprint_db: &mut re_data_store::LogDb, space_view_id: &SpaceViewId) {
    let entity_path = EntityPath::from(format!(
        "{}/{}",
        SpaceViewComponent::SPACEVIEW_PREFIX,
        space_view_id
    ));

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let cell =
        DataCell::from_arrow_empty(SpaceViewComponent::name(), SpaceViewComponent::data_type());

    let mut row = DataRow::from_cells1(RowId::random(), entity_path, timepoint, 0, cell);
    row.compute_all_size_bytes();

    match blueprint_db.entity_db.try_add_data_row(&row) {
        Ok(()) => {}
        Err(err) => {
            re_log::warn_once!("Failed to clear space view {}: {err}", space_view_id,);
        }
    }
}

pub fn sync_viewport(
    blueprint_db: &mut re_data_store::LogDb,
    viewport: &Viewport,
    snapshot: &Viewport,
) {
    let entity_path = EntityPath::from(VIEWPORT_PATH);

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    if viewport.auto_space_views != snapshot.auto_space_views {
        let component = AutoSpaceViews(viewport.auto_space_views);
        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }

    if viewport.visible != snapshot.visible {
        let component = SpaceViewVisibility(viewport.visible.clone());
        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }

    if viewport.maximized != snapshot.maximized {
        let component = SpaceViewMaximized(viewport.maximized);
        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }

    // Terrible hack since egui_dock::Tree does not implement PartialEq:
    // This could likely be improved using a crate like `serde_diff`, but we already
    // have the dep on `rmp_serde`, and hopefully this just goes away when we move
    // to `egui_tiles`.
    let mut tree_buff_viewport = Vec::new();
    rmp_serde::encode::write_named(&mut tree_buff_viewport, &viewport.trees).unwrap();
    let mut tree_buff_snapshot = Vec::new();
    rmp_serde::encode::write_named(&mut tree_buff_snapshot, &snapshot.trees).unwrap();
    if tree_buff_viewport != tree_buff_snapshot
        || viewport.has_been_user_edited != snapshot.has_been_user_edited
    {
        let component = ViewportLayout {
            space_view_keys: viewport.space_views.keys().cloned().collect(),
            trees: viewport.trees.clone(),
            has_been_user_edited: viewport.has_been_user_edited,
        };

        store_one_component(blueprint_db, &entity_path, &timepoint, component);

        // TODO(jleibs): Sort out this causality mess
        // If we are saving a new layout, we also need to save the visibility-set because
        // it gets mutated on load but isn't guaranteed to be mutated on layout-change
        // which means it won't get saved.
        let component = SpaceViewVisibility(viewport.visible.clone());
        store_one_component(blueprint_db, &entity_path, &timepoint, component);
    }
}
