use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ahash::HashMap;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind};
use re_types_core::reflection::Reflection;

use crate::{
    blueprint_timeline, command_channel, ApplicationSelectionState, CommandReceiver, CommandSender,
    ComponentUiRegistry, DataQueryResult, ItemCollection, RecordingConfig, StoreContext,
    SystemCommand, ViewClassRegistry, ViewId, ViewerContext,
};

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
    pub selection_state: ApplicationSelectionState,
    pub recording_config: RecordingConfig,

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
        // We rely a lot on logging in the viewer to identify issues.
        // Make sure logging is set up if it hasn't been done yet.
        let _ = env_logger::builder().is_test(true).try_init();

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

        Self {
            recording_store,
            blueprint_store,
            view_class_registry: Default::default(),
            selection_state: Default::default(),
            recording_config,
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
        )
        .expect("Failed to initialize re_renderer"),
    );

    render_state
}

/// Instance & adapter
struct SharedWgpuResources {
    instance: Arc<wgpu::Instance>,
    adapter: Arc<wgpu::Adapter>,
    device: Arc<wgpu::Device>,

    // Sharing the queue across parallel running tests should work fine in theory - it's obviously threadsafe.
    // Note though that this becomes an odd sync point that is shared with all tests that put in work here.
    queue: Arc<wgpu::Queue>,
}

static SHARED_WGPU_RENDERER_SETUP: Lazy<SharedWgpuResources> =
    Lazy::new(init_shared_renderer_setup);

fn init_shared_renderer_setup() -> SharedWgpuResources {
    // TODO(andreas, emilk/egui#5506): Use centralized wgpu setup logic that…
    // * lives mostly in re_renderer and is shared with viewer & renderer examples
    // * can be told to prefer software rendering
    // * can be told to match a specific device tier

    // We rely a lot on logging in the viewer to identify issues.
    // Make sure logging is set up if it hasn't been done yet.
    //let _ = env_logger::builder().is_test(true).try_init();
    let _ = env_logger::builder()
        .filter_level(re_log::external::log::LevelFilter::Trace)
        .is_test(false)
        .try_init();

    // We don't test on GL & DX12 right now (and don't want to do so by mistake!).
    // Several reasons for this:
    // * our CI is setup to draw with native Mac & lavapipe
    // * we generally prefer Vulkan over DX12 on Windows since it reduces the
    //   number of backends and wgpu's DX12 backend isn't as far along as of writing.
    // * we don't want to use the GL backend here since we regard it as a fallback only
    //   (TODO(andreas): Ideally we'd test that as well to check it is well-behaved,
    //   but for now we only want to have a look at the happy path)
    let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;
    let flags = (wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER
        | wgpu::InstanceFlags::VALIDATION
        | wgpu::InstanceFlags::GPU_BASED_VALIDATION)
        .with_env(); // Allow overwriting flags via env vars.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        flags,
        ..Default::default()
    });

    let mut adapters = instance.enumerate_adapters(backends);
    assert!(!adapters.is_empty(), "No graphics adapter found!");
    re_log::info!("Found the following adapters:");
    for adapter in &adapters {
        re_log::info!("* {}", egui_wgpu::adapter_info_summary(&adapter.get_info()));
    }

    // Adapters are already sorted by preferred backend by wgpu, but let's be explicit.
    adapters.sort_by_key(|a| match a.get_info().backend {
        wgpu::Backend::Metal => 0,
        wgpu::Backend::Vulkan => 1,
        wgpu::Backend::Dx12 => 2,
        wgpu::Backend::Gl => 4,
        wgpu::Backend::BrowserWebGpu => 6,
        wgpu::Backend::Empty => 7,
    });

    // Prefer CPU adapters, otherwise if we can't, prefer discrete GPU over integrated GPU.
    adapters.sort_by_key(|a| match a.get_info().device_type {
        wgpu::DeviceType::Cpu => 0, // CPU is the best for our purposes!
        wgpu::DeviceType::DiscreteGpu => 1,
        wgpu::DeviceType::Other
        | wgpu::DeviceType::IntegratedGpu
        | wgpu::DeviceType::VirtualGpu => 2,
    });

    let adapter = adapters.remove(0);
    re_log::info!("Picked adapter: {:?}", adapter.get_info());

    let device_caps = re_renderer::config::DeviceCaps::from_adapter(&adapter)
        .expect("Failed to determine device capabilities");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_caps.device_descriptor(), None))
            .expect("Failed to request device.");

    SharedWgpuResources {
        instance: Arc::new(instance),
        adapter: Arc::new(adapter),
        device: Arc::new(device),
        queue: Arc::new(queue),
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

    pub fn edit_selection(&mut self, edit_fn: impl FnOnce(&mut ApplicationSelectionState)) {
        edit_fn(&mut self.selection_state);

        // the selection state is double-buffered, so let's ensure it's updated
        self.selection_state.on_frame_start(|_| true, None);
    }

    /// Run the provided closure with a [`ViewerContext`] produced by the [`Self`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    pub fn run(&self, egui_ctx: &egui::Context, func: impl FnOnce(&ViewerContext<'_>)) {
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

        let drag_and_drop_manager = crate::DragAndDropManager::new(ItemCollection::default());

        let mut context_render_state = self.egui_render_state.lock();
        let render_state = context_render_state.get_or_insert_with(create_egui_renderstate);
        let mut egui_renderer = render_state.renderer.write();
        let render_ctx = egui_renderer
            .callback_resources
            .get_mut::<re_renderer::RenderContext>()
            .expect("No re_renderer::RenderContext in egui_render_state");
        render_ctx.begin_frame();

        let ctx = ViewerContext {
            app_options: &Default::default(),
            cache: &Default::default(),
            reflection: &self.reflection,
            component_ui_registry: &self.component_ui_registry,
            view_class_registry: &self.view_class_registry,
            store_context: &store_context,
            applicable_entities_per_visualizer: &Default::default(),
            indicated_entities_per_visualizer: &Default::default(),
            query_results: &self.query_results,
            rec_cfg: &self.recording_config,
            blueprint_cfg: &Default::default(),
            selection_state: &self.selection_state,
            blueprint_query: &self.blueprint_query,
            egui_ctx,
            render_ctx,
            command_sender: &self.command_sender,
            focused_item: &None,
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
    ///   [`egui::Context::run`].
    //TODO(ab): replace this with a kittest-based helper.
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
                    self.selection_state.set_selection(item);
                }

                SystemCommand::SetActiveTimeline { rec_id, timeline } => {
                    assert_eq!(rec_id, self.recording_store.store_id());
                    self.recording_config
                        .time_ctrl
                        .write()
                        .set_timeline(timeline);
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
                | SystemCommand::ClearActiveBlueprintAndEnableHeuristics
                | SystemCommand::ActivateRecording(_)
                | SystemCommand::CloseStore(_)
                | SystemCommand::UndoBlueprint { .. }
                | SystemCommand::RedoBlueprint { .. }
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

        test_context.run_in_egui_central_panel(|ctx, _| {
            assert_eq!(
                ctx.selection_state.selected_items().single_item(),
                Some(&item)
            );
        });
    }
}
