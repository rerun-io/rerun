//! The UI for the selection panel.

mod defaults_ui;
mod selection_history_ui;
mod selection_panel;
mod space_view_entity_picker;
mod space_view_space_origin_ui;
mod visible_time_range_ui;
mod visualizer_ui;

pub use selection_panel::SelectionPanel;

#[cfg(test)]
mod test {
    use super::*;
    use re_chunk_store::LatestAtQuery;
    use re_viewer_context::{blueprint_timeline, Item, SpaceViewId};
    use re_viewport_blueprint::ViewportBlueprint;

    /// This test mainly serve to demonstrate that non-trivial UI code can be executed with a "fake"
    /// [`ViewerContext`].
    // TODO(#6450): check that no warning/error is logged
    #[test]
    fn test_selection_panel() {
        re_log::setup_logging();

        let mut test_ctx = re_viewer_context::test_context::TestContext::default();
        test_ctx.edit_selection(|selection_state| {
            selection_state.set_selection(Item::SpaceView(SpaceViewId::random()));
        });

        test_ctx.run(|ctx, ui| {
            let (sender, _) = std::sync::mpsc::channel();
            let blueprint = ViewportBlueprint::try_from_db(
                ctx.store_context.blueprint,
                &LatestAtQuery::latest(blueprint_timeline()),
                sender,
            );

            let mut selection_panel = SelectionPanel::default();
            selection_panel.show_panel(ctx, &blueprint, &mut Default::default(), ui, true);
        });
    }
}
