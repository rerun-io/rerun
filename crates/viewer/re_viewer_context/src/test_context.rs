use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ahash::HashMap;
use egui::Context;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::{
    blueprint_timeline, command_channel, ApplicationSelectionState, CommandReceiver, CommandSender,
    ComponentUiRegistry, DataQueryResult, GlobalContext, ItemCollection, RecordingConfig,
    StoreContext, SystemCommand, ViewClass, ViewClassRegistry, ViewId, ViewStates, ViewerContext,
};
use re_chunk::{Chunk, ChunkBuilder};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, StoreKind, Timeline};
use re_types_core::reflection::Reflection;
use re_ui::Help;

pub trait HarnessExt {
    /// Fails the test iff more than `broken_percent_threshold`% pixels are broken.
    //
    // TODO(emilk/egui#5683): this should be natively supported by kittest
    fn snapshot_with_broken_pixels_threshold(
        &mut self,
        name: &str,
        num_pixels: u64,
        broken_percent_threshold: f64,
    );
}

impl HarnessExt for egui_kittest::Harness<'_> {
    fn snapshot_with_broken_pixels_threshold(
        &mut self,
        name: &str,
        num_pixels: u64,
        broken_percent_threshold: f64,
    ) {
        match self.try_snapshot(name) {
            Ok(_) => {}

            Err(err) => match err {
                egui_kittest::SnapshotError::Diff {
                    name,
                    diff: num_broken_pixels,
                    diff_path,
                } => {
                    let broken_percent = num_broken_pixels as f64 / num_pixels as f64;
                    re_log::debug!(num_pixels, num_broken_pixels, broken_percent);
                    assert!(
                        broken_percent <= broken_percent_threshold,
                        "{name} failed because {broken_percent} > {broken_percent_threshold}\n{diff_path:?}"
                    );
                }

                _ => panic!("{name} failed: {err}"),
            },
        }
    }
}

/// Harness to execute code that rely on [`crate::ViewerContext`].
///
/// Example:
/// ```rust
/// use re_viewer_context::test_context::TestContext;
/// use re_viewer_context::ViewerContext;
///
/// let mut test_context = TestContext::default();
/// test_context.run_in_egui_central_panel(|ctx: &ViewerContext, _| {
///     /* do something with ctx */
/// });
/// ```
pub struct TestContext {
    pub recording_store: EntityDb,
    pub blueprint_store: EntityDb,
    pub view_class_registry: ViewClassRegistry,

    // Mutex is needed, so we can update these from the `run` method
    pub selection_state: Mutex<ApplicationSelectionState>,
    pub focused_item: Mutex<Option<crate::Item>>,

    // Arc to make it easy to modify the time cursor at runtime (i.e. while the harness is running).
    pub recording_config: Arc<RecordingConfig>,
    pub view_states: Mutex<ViewStates>,

    // Populating this in `run` would pull in too many dependencies into the test harness for now.
    pub query_results: HashMap<ViewId, DataQueryResult>,

    pub blueprint_query: LatestAtQuery,
    pub component_ui_registry: ComponentUiRegistry,
    pub reflection: Reflection,

    command_sender: CommandSender,
    command_receiver: CommandReceiver,

    egui_render_state: Mutex<Option<egui_wgpu::RenderState>>,
    called_setup_kittest_for_rendering: AtomicBool,
}

impl Default for TestContext {
    fn default() -> Self {
        re_log::setup_logging();

        let recording_store = EntityDb::new(StoreId::random(StoreKind::Recording));
        let blueprint_store = EntityDb::new(StoreId::random(StoreKind::Blueprint));

        let (command_sender, command_receiver) = command_channel();

        let recording_config = RecordingConfig::default();

        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());

        let component_ui_registry = ComponentUiRegistry::new(Box::new(
            |_ctx, _ui, _ui_layout, _query, _db, _entity_path, _row_id, _component| {},
        ));

        let reflection =
            re_types::reflection::generate_reflection().expect("Failed to generate reflection");

        recording_config
            .time_ctrl
            .write()
            .set_timeline(Timeline::log_tick());

        Self {
            recording_store,
            blueprint_store,
            view_class_registry: Default::default(),
            selection_state: Default::default(),
            focused_item: Default::default(),
            recording_config: Arc::new(recording_config),
            view_states: Default::default(),
            blueprint_query,
            query_results: Default::default(),
            component_ui_registry,
            reflection,
            command_sender,
            command_receiver,

            // Created lazily since each egui_kittest harness needs a new one.
            egui_render_state: Mutex::new(None),
            called_setup_kittest_for_rendering: AtomicBool::new(false),
        }
    }
}

/// Create an `egui_wgpu::RenderState` for tests.
fn create_egui_renderstate() -> egui_wgpu::RenderState {
    re_tracing::profile_function!();

    let shared_wgpu_setup = &*SHARED_WGPU_RENDERER_SETUP;

    let config = egui_wgpu::WgpuConfiguration {
        wgpu_setup: egui_wgpu::WgpuSetupExisting {
            instance: shared_wgpu_setup.instance.clone(),
            adapter: shared_wgpu_setup.adapter.clone(),
            device: shared_wgpu_setup.device.clone(),
            queue: shared_wgpu_setup.queue.clone(),
        }
        .into(),

        // None of these matter for tests as we're not going to draw to a surfaces.
        present_mode: wgpu::PresentMode::Immediate,
        desired_maximum_frame_latency: None,
        on_surface_error: Arc::new(|_| {
            unreachable!("tests aren't expected to draw to surfaces");
        }),
    };

    let compatible_surface = None;
    // `re_renderer`'s individual views (managed each by a `ViewBuilder`) have MSAA,
    // but egui's final target doesn't - re_renderer resolves and copies into egui in `ViewBuilder::composite`.
    let msaa_samples = 1;
    // Similarly, depth is handled by re_renderer.
    let depth_format = None;
    // Disable dithering in order to not unnecessarily add a source of noise & variance between renderers.
    let dithering = false;

    let render_state = pollster::block_on(egui_wgpu::RenderState::create(
        &config,
        &shared_wgpu_setup.instance,
        compatible_surface,
        depth_format,
        msaa_samples,
        dithering,
    ))
    .expect("Failed to set up egui_wgpu::RenderState");

    // Put re_renderer::RenderContext into the callback resources so that render callbacks can access it.
    render_state.renderer.write().callback_resources.insert(
        re_renderer::RenderContext::new(
            &shared_wgpu_setup.adapter,
            shared_wgpu_setup.device.clone(),
            shared_wgpu_setup.queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
            |_| re_renderer::RenderConfig::testing(),
        )
        .expect("Failed to initialize re_renderer"),
    );

    render_state
}

/// Instance & adapter
struct SharedWgpuResources {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,

    // Sharing the queue across parallel running tests should work fine in theory - it's obviously threadsafe.
    // Note though that this becomes an odd sync point that is shared with all tests that put in work here.
    queue: wgpu::Queue,
}

static SHARED_WGPU_RENDERER_SETUP: Lazy<SharedWgpuResources> =
    Lazy::new(init_shared_renderer_setup);

fn init_shared_renderer_setup() -> SharedWgpuResources {
    let instance = wgpu::Instance::new(&re_renderer::device_caps::testing_instance_descriptor());
    let adapter = re_renderer::device_caps::select_testing_adapter(&instance);
    let device_caps = re_renderer::device_caps::DeviceCaps::from_adapter(&adapter)
        .expect("Failed to determine device capabilities");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_caps.device_descriptor(), None))
            .expect("Failed to request device.");

    SharedWgpuResources {
        instance,
        adapter,
        device,
        queue,
    }
}

impl TestContext {
    pub fn setup_kittest_for_rendering(&self) -> egui_kittest::HarnessBuilder<()> {
        // Egui kittests insists on having a fresh render state for each test.
        let new_render_state = create_egui_renderstate();
        let builder = egui_kittest::Harness::builder().renderer(
            // Note that render state clone is mostly cloning of inner `Arc`.
            // This does _not_ duplicate re_renderer's context contained within.
            egui_kittest::wgpu::WgpuTestRenderer::from_render_state(new_render_state.clone()),
        );
        self.egui_render_state.lock().replace(new_render_state);

        self.called_setup_kittest_for_rendering
            .store(true, std::sync::atomic::Ordering::Relaxed);

        builder
    }

    /// Timeline the recording config is using by default.
    pub fn active_timeline(&self) -> re_chunk::Timeline {
        *self.recording_config.time_ctrl.read().timeline()
    }

    pub fn set_active_timeline(&self, timeline: Timeline) {
        self.recording_config
            .time_ctrl
            .write()
            .set_timeline(timeline);
    }

    pub fn edit_selection(&self, edit_fn: impl FnOnce(&mut ApplicationSelectionState)) {
        let mut selection_state = self.selection_state.lock();
        edit_fn(&mut selection_state);

        // the selection state is double-buffered, so let's ensure it's updated
        selection_state.on_frame_start(|_| true, None);
    }

    /// Log an entity to the recording store.
    ///
    /// The provided closure should add content using the [`ChunkBuilder`] passed as argument.
    pub fn log_entity(
        &mut self,
        entity_path: EntityPath,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    ) {
        let builder = build_chunk(Chunk::builder(entity_path));
        self.recording_store
            .add_chunk(&Arc::new(
                builder.build().expect("chunk should be successfully built"),
            ))
            .expect("chunk should be successfully added");
    }

    /// Register a view class.
    pub fn register_view_class<T: ViewClass + Default + 'static>(&mut self) {
        self.view_class_registry
            .add_class::<T>()
            .expect("registering a class should succeed");
    }

    /// Run the provided closure with a [`ViewerContext`] produced by the [`Self`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    pub fn run(&self, egui_ctx: &egui::Context, func: impl FnOnce(&ViewerContext<'_>)) {
        re_log::PanicOnWarnScope::new(); // TODO(andreas): There should be a way to opt-out of this.
        re_ui::apply_style_and_install_loaders(egui_ctx);

        let store_context = StoreContext {
            app_id: "rerun_test".into(),
            blueprint: &self.blueprint_store,
            default_blueprint: None,
            recording: &self.recording_store,
            bundle: &Default::default(),
            caches: &Default::default(),
            hub: &Default::default(),
            should_enable_heuristics: false,
        };
        let indicated_entities_per_visualizer = self
            .view_class_registry
            .indicated_entities_per_visualizer(&store_context.recording.store_id());

        let drag_and_drop_manager = crate::DragAndDropManager::new(ItemCollection::default());

        let mut context_render_state = self.egui_render_state.lock();
        let render_state = context_render_state.get_or_insert_with(create_egui_renderstate);
        let mut egui_renderer = render_state.renderer.write();
        let render_ctx = egui_renderer
            .callback_resources
            .get_mut::<re_renderer::RenderContext>()
            .expect("No re_renderer::RenderContext in egui_render_state");
        render_ctx.begin_frame();

        let mut selection_state = self.selection_state.lock();
        let mut focused_item = self.focused_item.lock();

        let ctx = ViewerContext {
            global_context: GlobalContext {
                app_options: &Default::default(),
                reflection: &self.reflection,
                component_ui_registry: &self.component_ui_registry,
                view_class_registry: &self.view_class_registry,
                egui_ctx,
                command_sender: &self.command_sender,
                render_ctx,
            },
            store_context: &store_context,
            maybe_visualizable_entities_per_visualizer: &Default::default(),
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &self.query_results,
            rec_cfg: &self.recording_config,
            blueprint_cfg: &Default::default(),
            selection_state: &selection_state,
            blueprint_query: &self.blueprint_query,
            focused_item: &focused_item,
            drag_and_drop_manager: &drag_and_drop_manager,
        };

        func(&ctx);

        // If re_renderer was used, `setup_kittest_for_rendering` should have been called.
        let num_view_builders_created = render_ctx.active_frame.num_view_builders_created();
        let called_setup_kittest_for_rendering = self
            .called_setup_kittest_for_rendering
            .load(std::sync::atomic::Ordering::Relaxed);
        assert!(num_view_builders_created == 0 || called_setup_kittest_for_rendering,
                "Rendering with `re_renderer` requires setting up kittest with `TestContext::setup_kittest_for_rendering`
                to ensure that kittest & re_renderer use the same graphics device.");

        render_ctx.before_submit();

        selection_state.on_frame_start(|_| true, None);
        *focused_item = None;
    }

    /// Run the given function with a [`ViewerContext`] produced by the [`Self`], in the context of
    /// an [`egui::CentralPanel`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    ///
    /// Notes:
    /// - Uses [`egui::__run_test_ctx`].
    /// - There is a possibility that the closure will be called more than once, see
    ///   [`egui::Context::run`]. Use [`Self::run_once_in_egui_central_panel`] if you want to ensure
    ///   that the closure is called exactly once.
    //TODO(ab): should this be removed entirely in favor of `run_once_in_egui_central_panel`?
    pub fn run_in_egui_central_panel(
        &self,
        mut func: impl FnMut(&ViewerContext<'_>, &mut egui::Ui),
    ) {
        egui::__run_test_ctx(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let egui_ctx = ui.ctx().clone();

                self.run(&egui_ctx, |ctx| {
                    func(ctx, ui);
                });
            });
        });
    }

    /// Run the given function once with a [`ViewerContext`] produced by the [`Self`], in the
    /// context of an [`egui::CentralPanel`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    ///
    /// Notes:
    /// - Uses [`egui::__run_test_ctx`].
    pub fn run_once_in_egui_central_panel<R>(
        &self,
        func: impl FnOnce(&ViewerContext<'_>, &mut egui::Ui) -> R,
    ) -> R {
        let mut func = Some(func);
        let mut result = None;

        egui::__run_test_ctx(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let egui_ctx = ui.ctx().clone();

                self.run(&egui_ctx, |ctx| {
                    if let Some(func) = func.take() {
                        result = Some(func(ctx, ui));
                    }
                });
            });
        });

        result.expect("Function should have been called at least once")
    }

    /// Best-effort attempt to meaningfully handle some of the system commands.
    pub fn handle_system_commands(&mut self) {
        while let Some(command) = self.command_receiver.recv_system() {
            let mut handled = true;
            let command_name = format!("{command:?}");
            match command {
                SystemCommand::UpdateBlueprint(store_id, chunks) => {
                    assert_eq!(store_id, self.blueprint_store.store_id());

                    for chunk in chunks {
                        self.blueprint_store
                            .add_chunk(&Arc::new(chunk))
                            .expect("Updating the blueprint chunk store failed");
                    }
                }

                SystemCommand::DropEntity(store_id, entity_path) => {
                    assert_eq!(store_id, self.blueprint_store.store_id());
                    self.blueprint_store
                        .drop_entity_path_recursive(&entity_path);
                }

                SystemCommand::SetSelection(item) => {
                    self.selection_state.lock().set_selection(item);
                }

                SystemCommand::SetFocus(item) => {
                    *self.focused_item.lock() = Some(item);
                }

                SystemCommand::SetActiveTimeline { rec_id, timeline } => {
                    assert_eq!(rec_id, self.recording_store.store_id());
                    self.recording_config
                        .time_ctrl
                        .write()
                        .set_timeline(timeline);
                }

                // not implemented
                SystemCommand::ActivateApp(_)
                | SystemCommand::CloseApp(_)
                | SystemCommand::LoadDataSource(_)
                | SystemCommand::ClearSourceAndItsStores(_)
                | SystemCommand::AddReceiver { .. }
                | SystemCommand::ResetViewer
                | SystemCommand::ChangeDisplayMode(_)
                | SystemCommand::ClearActiveBlueprint
                | SystemCommand::ClearActiveBlueprintAndEnableHeuristics
                | SystemCommand::AddRedapServer { .. }
                | SystemCommand::SelectRedapEntry { .. }
                | SystemCommand::SelectRedapServer { .. }
                | SystemCommand::ActivateRecording(_)
                | SystemCommand::CloseStore(_)
                | SystemCommand::UndoBlueprint { .. }
                | SystemCommand::RedoBlueprint { .. }
                | SystemCommand::CloseAllRecordings
                | SystemCommand::SetLoopSelection { .. } => handled = false,

                #[cfg(debug_assertions)]
                SystemCommand::EnableInspectBlueprintTimeline(_) => handled = false,

                #[cfg(not(target_arch = "wasm32"))]
                SystemCommand::FileSaver(_) => handled = false,
            }

            if !handled {
                eprintln!("Ignored system command: {command_name:?}",);
            }
        }
    }

    pub fn test_help_view(help: impl Fn(&Context) -> Help) {
        use egui::os::OperatingSystem;
        for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
            let mut harness = egui_kittest::Harness::builder().build_ui(|ui| {
                ui.ctx().set_os(os);
                re_ui::apply_style_and_install_loaders(ui.ctx());
                help(ui.ctx()).ui(ui);
            });
            let help_view = help(&harness.ctx);
            let name = format!("help_view_{}_{os:?}", help_view.title())
                .replace(' ', "_")
                .to_lowercase();
            harness.fit_contents();
            harness.snapshot(&name);
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
        let test_context = TestContext::default();

        let item = Item::InstancePath(InstancePath::entity_all("/entity/path".into()));

        test_context.edit_selection(|selection_state| {
            selection_state.set_selection(item.clone());
        });

        test_context.run_in_egui_central_panel(|ctx, _| {
            assert_eq!(
                ctx.selection_state.selected_items().single_item(),
                Some(&item)
            );
        });
    }
}
