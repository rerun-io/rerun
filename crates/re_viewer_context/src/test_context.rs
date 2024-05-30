use crate::{
    command_channel, ApplicationSelectionState, ComponentUiRegistry, StoreContext, ViewerContext,
};

use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind};

/// Harness to execute code that rely on [`crate::ViewerContext`].
///
/// Example:
/// ```rust
/// use re_viewer_context::test_context::TestContext;
/// use re_viewer_context::ViewerContext;
///
/// let mut test_context = TestContext::default();
/// test_context.run(|ctx: &ViewerContext, _| {
///     /* do something with ctx */
/// });
/// ```
pub struct TestContext {
    recording_store: EntityDb,
    blueprint_store: EntityDb,
    selection_state: ApplicationSelectionState,
}

impl Default for TestContext {
    fn default() -> Self {
        let recording_store = EntityDb::new(StoreId::random(StoreKind::Recording));
        let blueprint_store = EntityDb::new(StoreId::random(StoreKind::Blueprint));
        Self {
            recording_store,
            blueprint_store,
            selection_state: Default::default(),
        }
    }
}

impl TestContext {
    pub fn edit_selection(&mut self, edit_fn: impl FnOnce(&mut ApplicationSelectionState)) {
        edit_fn(&mut self.selection_state);

        // the selection state is double-buffered, so let's ensure it's updated
        self.selection_state.on_frame_start(|_| true, None);
    }

    pub fn run(&self, mut func: impl FnMut(&ViewerContext<'_>, &mut egui::Ui)) {
        egui::__run_test_ui(|ui| {
            let re_ui = re_ui::ReUi::load_and_apply(ui.ctx());
            let blueprint_query = LatestAtQuery::latest(re_log_types::Timeline::new(
                "timeline",
                re_log_types::TimeType::Time,
            ));
            let (command_sender, _) = command_channel();
            let component_ui_registry = ComponentUiRegistry::new(Box::new(
                |_ctx, _ui, _ui_layout, _query, _db, _entity_path, _component, _instance| {},
            ));

            let store_context = StoreContext {
                app_id: "rerun_test".into(),
                blueprint: &self.blueprint_store,
                default_blueprint: None,
                recording: &self.recording_store,
                bundle: &Default::default(),
                hub: &Default::default(),
            };

            let ctx = ViewerContext {
                app_options: &Default::default(),
                cache: &Default::default(),
                component_ui_registry: &component_ui_registry,
                space_view_class_registry: &Default::default(),
                store_context: &store_context,
                applicable_entities_per_visualizer: &Default::default(),
                indicated_entities_per_visualizer: &Default::default(),
                query_results: &Default::default(),
                rec_cfg: &Default::default(),
                blueprint_cfg: &Default::default(),
                selection_state: &self.selection_state,
                blueprint_query: &blueprint_query,
                re_ui: &re_ui,
                render_ctx: None,
                command_sender: &command_sender,
                focused_item: &None,
                component_base_fallbacks: &Default::default(),
            };

            func(&ctx, ui);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Item;
    use re_entity_db::InstancePath;

    /// Test that `TestContext:edit_selection` works as expected, aka. its side effects are visible
    /// from `TestContext::run`.
    #[test]
    fn test_edit_selection() {
        let mut test_context = TestContext::default();

        let item = Item::InstancePath(InstancePath::entity_all("/entity/path".into()));

        test_context.edit_selection(|selection_state| {
            selection_state.set_selection(item.clone());
        });

        test_context.run(|ctx, _| {
            assert_eq!(
                ctx.selection_state.selected_items().single_item(),
                Some(&item)
            );
        });
    }
}
