use std::sync::Arc;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind, Timeline};

use crate::{
    blueprint_timeline, command_channel, ApplicationSelectionState, CommandReceiver, CommandSender,
    ComponentUiRegistry, RecordingConfig, SpaceViewClassRegistry, StoreContext, SystemCommand,
    ViewerContext,
};

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
    pub recording_store: EntityDb,
    pub blueprint_store: EntityDb,
    pub space_view_class_registry: SpaceViewClassRegistry,
    pub selection_state: ApplicationSelectionState,
    pub active_timeline: Timeline,

    command_sender: CommandSender,
    command_receiver: CommandReceiver,
}

impl Default for TestContext {
    fn default() -> Self {
        let recording_store = EntityDb::new(StoreId::random(StoreKind::Recording));
        let blueprint_store = EntityDb::new(StoreId::random(StoreKind::Blueprint));
        let active_timeline = Timeline::new("time", re_log_types::TimeType::Time);

        let (command_sender, command_receiver) = command_channel();
        Self {
            recording_store,
            blueprint_store,
            space_view_class_registry: Default::default(),
            selection_state: Default::default(),
            active_timeline,
            command_sender,
            command_receiver,
        }
    }
}

impl TestContext {
    pub fn edit_selection(&mut self, edit_fn: impl FnOnce(&mut ApplicationSelectionState)) {
        edit_fn(&mut self.selection_state);

        // the selection state is double-buffered, so let's ensure it's updated
        self.selection_state.on_frame_start(|_| true, None);
    }

    /// Run the given function with a [`ViewerContext`] produced by the [`Self`].
    ///
    /// Note: there is a possibility that the closure will be called more than once, see
    /// [`egui::Context::run`].
    pub fn run(&self, mut func: impl FnMut(&ViewerContext<'_>, &mut egui::Ui)) {
        egui::__run_test_ctx(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                let blueprint_query = LatestAtQuery::latest(blueprint_timeline());

                let component_ui_registry = ComponentUiRegistry::new(Box::new(
                    |_ctx, _ui, _ui_layout, _query, _db, _entity_path, _row_id, _component| {},
                ));

                let store_context = StoreContext {
                    app_id: "rerun_test".into(),
                    blueprint: &self.blueprint_store,
                    default_blueprint: None,
                    recording: &self.recording_store,
                    bundle: &Default::default(),
                    caches: &Default::default(),
                    hub: &Default::default(),
                };

                let rec_cfg = RecordingConfig::default();
                rec_cfg.time_ctrl.write().set_timeline(self.active_timeline);

                let egui_context = ui.ctx().clone();
                let ctx = ViewerContext {
                    app_options: &Default::default(),
                    cache: &Default::default(),
                    reflection: &Default::default(),
                    component_ui_registry: &component_ui_registry,
                    space_view_class_registry: &self.space_view_class_registry,
                    store_context: &store_context,
                    applicable_entities_per_visualizer: &Default::default(),
                    indicated_entities_per_visualizer: &Default::default(),
                    query_results: &Default::default(),
                    rec_cfg: &rec_cfg,
                    blueprint_cfg: &Default::default(),
                    selection_state: &self.selection_state,
                    blueprint_query: &blueprint_query,
                    egui_ctx: &egui_context,
                    render_ctx: None,
                    command_sender: &self.command_sender,
                    focused_item: &None,
                };

                func(&ctx, ui);
            });
        });
    }

    /// Run the given function with a [`ViewerContext`] produced by the [`Self`] and handle any
    /// system commands issued during execution (see [`Self::handle_system_command`]).
    pub fn run_and_handle_system_commands(
        &mut self,
        func: impl FnMut(&ViewerContext<'_>, &mut egui::Ui),
    ) {
        self.run(func);
        self.handle_system_command();
    }

    /// Best-effort attempt to meaningfully handle some of the system commands.
    pub fn handle_system_command(&mut self) {
        while let Some(command) = self.command_receiver.recv_system() {
            let mut handled = true;
            let command_name = format!("{command:?}");
            match command {
                SystemCommand::UpdateBlueprint(store_id, chunks) => {
                    assert_eq!(&store_id, self.blueprint_store.store_id());

                    for chunk in chunks {
                        self.blueprint_store
                            .add_chunk(&Arc::new(chunk))
                            .expect("Updating the blueprint chunk store failed");
                    }
                }

                SystemCommand::DropEntity(store_id, entity_path) => {
                    assert_eq!(&store_id, self.blueprint_store.store_id());
                    self.blueprint_store
                        .drop_entity_path_recursive(&entity_path);
                }

                SystemCommand::SetSelection(item) => {
                    self.selection_state.set_selection(item);
                }

                SystemCommand::SetActiveTimeline { rec_id, timeline } => {
                    assert_eq!(&rec_id, self.recording_store.store_id());
                    self.active_timeline = timeline;
                }

                // not implemented
                SystemCommand::SetFocus(_)
                | SystemCommand::ActivateApp(_)
                | SystemCommand::CloseApp(_)
                | SystemCommand::LoadDataSource(_)
                | SystemCommand::ClearSourceAndItsStores(_)
                | SystemCommand::AddReceiver(_)
                | SystemCommand::ResetViewer
                | SystemCommand::ClearActiveBlueprint
                | SystemCommand::ClearAndGenerateBlueprint
                | SystemCommand::ActivateRecording(_)
                | SystemCommand::CloseStore(_)
                | SystemCommand::CloseAllRecordings => handled = false,

                #[cfg(debug_assertions)]
                SystemCommand::EnableInspectBlueprintTimeline(_) => handled = false,

                #[cfg(not(target_arch = "wasm32"))]
                SystemCommand::FileSaver(_) => handled = false,
            }

            eprintln!(
                "{} system command: {command_name:?}",
                if handled { "Handled" } else { "Ignored" }
            );
        }
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
