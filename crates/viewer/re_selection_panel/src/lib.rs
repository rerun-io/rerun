//! The UI for the selection panel.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod defaults_ui;
mod item_heading_no_breadcrumbs;
mod item_heading_with_breadcrumbs;
mod item_title;
mod selection_panel;
mod view_entity_picker;
mod view_space_origin_ui;
mod visible_time_range_ui;
mod visualizer_ui;

pub use selection_panel::SelectionPanel;

/// Whether to show the component mappings UI.
// TODO(RR-3338): Enable component mappings UI
pub(crate) const ENABLE_COMPONENT_MAPPINGS_UI: bool = false;

#[cfg(test)]
mod test {
    use re_chunk_store::LatestAtQuery;
    use re_viewer_context::{Item, ViewId, blueprint_timeline};
    use re_viewport_blueprint::ViewportBlueprint;

    use super::*;

    /// This test mainly serve to demonstrate that non-trivial UI code can be executed with a "fake"
    /// [`ViewerContext`].
    // TODO(#6450): check that no warning/error is logged
    #[test]
    fn test_selection_panel() {
        re_log::setup_logging();

        let test_ctx = re_test_context::TestContext::new();
        test_ctx.edit_selection(|selection_state| {
            selection_state.set_selection(Item::View(ViewId::random()));
        });

        test_ctx.run_in_egui_central_panel(|ctx, ui| {
            let blueprint = ViewportBlueprint::from_db(
                ctx.store_context.blueprint,
                &LatestAtQuery::latest(blueprint_timeline()),
            );

            let mut selection_panel = SelectionPanel::default();
            selection_panel.show_panel(ctx, &blueprint, &mut Default::default(), ui, true);
        });
    }
}
