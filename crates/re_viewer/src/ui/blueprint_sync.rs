use arrow2_convert::field::ArrowField;
use re_log_types::{
    Component, DataCell, DataRow, EntityPath, RowId, SerializableComponent, TimePoint,
};
use re_viewer_context::SpaceViewId;
use re_viewport::{
    blueprint_components::{
        AutoSpaceViews, SpaceViewComponent, SpaceViewMaximized, SpaceViewVisibility,
        ViewportLayout, VIEWPORT_PATH,
    },
    SpaceViewBlueprint, Viewport,
};

use super::Blueprint;

pub fn push_one_component<C: SerializableComponent>(
    deltas: &mut Vec<DataRow>,
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

    deltas.push(row);
}

// Resolving and applying updates
impl<'a> Blueprint<'a> {
    pub fn compute_deltas(&self, snapshot: &Self) -> Vec<DataRow> {
        let mut deltas = vec![];

        sync_viewport(&mut deltas, &self.viewport, &snapshot.viewport);

        // Add any new or modified space views
        for id in self.viewport.space_view_ids() {
            if let Some(space_view) = self.viewport.space_view(id) {
                sync_space_view(&mut deltas, space_view, snapshot.viewport.space_view(id));
            }
        }

        // Remove any deleted space views
        for space_view_id in snapshot.viewport.space_view_ids() {
            if self.viewport.space_view(space_view_id).is_none() {
                clear_space_view(&mut deltas, space_view_id);
            }
        }

        deltas
    }
}

pub fn sync_space_view(
    deltas: &mut Vec<DataRow>,
    space_view: &SpaceViewBlueprint,
    snapshot: Option<&SpaceViewBlueprint>,
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

        push_one_component(deltas, &entity_path, &timepoint, component);
    }
}

pub fn clear_space_view(deltas: &mut Vec<DataRow>, space_view_id: &SpaceViewId) {
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

    deltas.push(row);
}

pub fn sync_viewport(deltas: &mut Vec<DataRow>, viewport: &Viewport, snapshot: &Viewport) {
    let entity_path = EntityPath::from(VIEWPORT_PATH);

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    if viewport.auto_space_views != snapshot.auto_space_views {
        let component = AutoSpaceViews(viewport.auto_space_views);
        push_one_component(deltas, &entity_path, &timepoint, component);
    }

    if viewport.visible != snapshot.visible {
        let component = SpaceViewVisibility(viewport.visible.clone());
        push_one_component(deltas, &entity_path, &timepoint, component);
    }

    if viewport.maximized != snapshot.maximized {
        let component = SpaceViewMaximized(viewport.maximized);
        push_one_component(deltas, &entity_path, &timepoint, component);
    }

    // Note: we can't just check `viewport.trees != snapshot.trees` because the
    // tree contains serde[skip]'d state that won't match in PartialEq.
    if viewport.trees.len() != snapshot.trees.len()
        || !viewport.trees.iter().zip(snapshot.trees.iter()).all(
            |((left_vis, left_tree), (right_vis, right_tree))| {
                left_vis == right_vis
                    && left_tree.root == right_tree.root
                    && left_tree.tiles.tiles == right_tree.tiles.tiles
            },
        )
        || viewport.has_been_user_edited != snapshot.has_been_user_edited
    {
        let component = ViewportLayout {
            space_view_keys: viewport.space_views.keys().cloned().collect(),
            trees: viewport.trees.clone(),
            has_been_user_edited: viewport.has_been_user_edited,
        };

        push_one_component(deltas, &entity_path, &timepoint, component);

        // TODO(jleibs): Sort out this causality mess
        // If we are saving a new layout, we also need to save the visibility-set because
        // it gets mutated on load but isn't guaranteed to be mutated on layout-change
        // which means it won't get saved.
        let component = SpaceViewVisibility(viewport.visible.clone());
        push_one_component(deltas, &entity_path, &timepoint, component);
    }
}
