//! Provides a test context that builds on `re_viewer_context`.

#![expect(clippy::unwrap_used)] // This is only a test

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ahash::HashMap;
use egui::os::OperatingSystem;
use parking_lot::{Mutex, RwLock};
use re_chunk::{Chunk, ChunkBuilder};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{EntityPath, EntityPathPart, SetStoreInfo, StoreId, StoreInfo, StoreKind};
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::{Component as _, ComponentDescriptor};
use re_types_core::reflection::Reflection;
use re_ui::Help;
use re_viewer_context::{
    AppOptions, ApplicationSelectionState, BlueprintContext, CommandReceiver, CommandSender,
    ComponentUiRegistry, DataQueryResult, DisplayMode, FallbackProviderRegistry, GlobalContext,
    Item, ItemCollection, NeedsRepaint, StoreHub, SystemCommand, SystemCommandSender as _,
    TimeControl, TimeControlCommand, ViewClass, ViewClassRegistry, ViewId, ViewStates,
    ViewerContext, blueprint_timeline, command_channel,
};

pub mod external {
    pub use egui_kittest;
}

/// Harness to execute code that rely on [`crate::ViewerContext`].
///
/// Example:
/// ```rust
/// use re_test_context::TestContext;
/// use re_viewer_context::ViewerContext;
///
/// let mut test_context = TestContext::new();
/// test_context.run_in_egui_central_panel(|ctx: &ViewerContext, _| {
///     /* do something with ctx */
/// });
///
/// // To get proper UI:s, also run this:
/// // test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
/// // re_data_ui::register_component_uis(&mut test_context.component_ui_registry);
/// ```
pub struct TestContext {
    pub app_options: AppOptions,

    /// Store hub prepopulated with a single active recording & a blueprint recording.
    pub store_hub: Mutex<StoreHub>,
    pub view_class_registry: ViewClassRegistry,

    // Mutex is needed, so we can update these from the `run` method
    pub selection_state: Mutex<ApplicationSelectionState>,
    pub focused_item: Mutex<Option<re_viewer_context::Item>>,

    // RwLock so we can have `handle_system_commands` take an immutable reference to self.
    pub time_ctrl: RwLock<TimeControl>,
    pub view_states: Mutex<ViewStates>,

    // Populating this in `run` would pull in too many dependencies into the test harness for now.
    pub query_results: HashMap<ViewId, DataQueryResult>,

    pub blueprint_query: LatestAtQuery,
    pub component_ui_registry: ComponentUiRegistry,
    pub component_fallback_registry: FallbackProviderRegistry,
    pub reflection: Reflection,

    pub connection_registry: re_redap_client::ConnectionRegistryHandle,

    command_sender: CommandSender,
    command_receiver: CommandReceiver,

    egui_render_state: Mutex<Option<egui_wgpu::RenderState>>,
    called_setup_kittest_for_rendering: AtomicBool,
}

pub struct TestBlueprintCtx<'a> {
    command_sender: &'a CommandSender,
    current_blueprint: &'a EntityDb,
    default_blueprint: Option<&'a EntityDb>,
    blueprint_query: &'a re_chunk::LatestAtQuery,
}

impl BlueprintContext for TestBlueprintCtx<'_> {
    fn command_sender(&self) -> &CommandSender {
        self.command_sender
    }

    fn current_blueprint(&self) -> &EntityDb {
        self.current_blueprint
    }

    fn default_blueprint(&self) -> Option<&EntityDb> {
        self.default_blueprint
    }

    fn blueprint_query(&self) -> &re_chunk::LatestAtQuery {
        self.blueprint_query
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TestContext {
    pub fn new() -> Self {
        Self::new_with_store_info(StoreInfo::testing())
    }

    pub fn new_with_store_info(store_info: StoreInfo) -> Self {
        re_log::setup_logging();

        let application_id = store_info.application_id().clone();
        let recording_store_id = store_info.store_id.clone();
        let mut recording_store = EntityDb::new(recording_store_id.clone());

        recording_store.set_store_info(SetStoreInfo {
            row_id: Tuid::new(),
            info: store_info,
        });
        {
            // Set RecordingInfo:
            recording_store
                .set_recording_property(
                    EntityPath::properties(),
                    RecordingInfo::descriptor_name(),
                    &re_sdk_types::components::Name::from("Test recording"),
                )
                .unwrap();
            recording_store
                .set_recording_property(
                    EntityPath::properties(),
                    RecordingInfo::descriptor_start_time(),
                    &re_sdk_types::components::Timestamp::from(
                        "2025-06-28T19:26:42Z"
                            .parse::<jiff::Timestamp>()
                            .unwrap()
                            .as_nanosecond() as i64,
                    ),
                )
                .unwrap();
        }
        {
            // Set some custom recording properties:
            recording_store
                .set_recording_property(
                    EntityPath::properties() / EntityPathPart::from("episode"),
                    ComponentDescriptor {
                        archetype: None,
                        component: "location".into(),
                        component_type: Some(re_sdk_types::components::Text::name()),
                    },
                    &re_sdk_types::components::Text::from("Swallow Falls"),
                )
                .unwrap();
            recording_store
                .set_recording_property(
                    EntityPath::properties() / EntityPathPart::from("episode"),
                    ComponentDescriptor {
                        archetype: None,
                        component: "weather".into(),
                        component_type: Some(re_sdk_types::components::Text::name()),
                    },
                    &re_sdk_types::components::Text::from("Cloudy with meatballs"),
                )
                .unwrap();
        }

        let blueprint_id = StoreId::random(StoreKind::Blueprint, application_id);
        let blueprint_store = EntityDb::new(blueprint_id.clone());

        let mut store_hub = StoreHub::test_hub();
        store_hub.insert_entity_db(recording_store);
        store_hub.insert_entity_db(blueprint_store);
        store_hub.set_active_recording_id(recording_store_id);
        store_hub
            .set_cloned_blueprint_active_for_app(&blueprint_id)
            .expect("Failed to set blueprint as active");

        let (command_sender, command_receiver) = command_channel();

        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());

        let time_ctrl = {
            let ctx = TestBlueprintCtx {
                command_sender: &command_sender,
                current_blueprint: store_hub
                    .active_blueprint()
                    .expect("We should have an active blueprint now"),
                default_blueprint: store_hub.default_blueprint_for_app(
                    store_hub
                        .active_app()
                        .expect("We should have an active app now"),
                ),
                blueprint_query: &blueprint_query,
            };

            TimeControl::from_blueprint(&ctx)
        };

        let component_ui_registry = ComponentUiRegistry::new();

        let component_fallback_registry =
            re_component_fallbacks::create_component_fallback_registry();

        let reflection =
            re_sdk_types::reflection::generate_reflection().expect("Failed to generate reflection");

        Self {
            app_options: Default::default(),

            view_class_registry: Default::default(),
            selection_state: Default::default(),
            focused_item: Default::default(),
            time_ctrl: RwLock::new(time_ctrl),
            view_states: Default::default(),
            blueprint_query,
            query_results: Default::default(),
            component_ui_registry,
            component_fallback_registry,
            reflection,
            connection_registry:
                re_redap_client::ConnectionRegistry::new_without_stored_credentials(),

            command_sender,
            command_receiver,

            // Created lazily since each egui_kittest harness needs a new one.
            egui_render_state: Mutex::new(None),
            called_setup_kittest_for_rendering: AtomicBool::new(false),

            store_hub: Mutex::new(store_hub),
        }
    }

    /// Create a new test context that knows about a specific view class.
    ///
    /// This is useful for tests that need test a single view class.
    ///
    /// Note that it's important to first register the view class before adding any entities,
    /// otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    /// and thus will not find anything applicable to the visualizer.
    pub fn new_with_view_class<T: ViewClass + Default + 'static>() -> Self {
        let mut test_context = Self::new();
        test_context.register_view_class::<T>();
        test_context
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

    let render_state = pollster::block_on(egui_wgpu::RenderState::create(
        &config,
        &shared_wgpu_setup.instance,
        compatible_surface,
        egui_wgpu::RendererOptions::PREDICTABLE,
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

static SHARED_WGPU_RENDERER_SETUP: std::sync::LazyLock<SharedWgpuResources> =
    std::sync::LazyLock::new(init_shared_renderer_setup);

fn init_shared_renderer_setup() -> SharedWgpuResources {
    let instance = wgpu::Instance::new(&re_renderer::device_caps::testing_instance_descriptor());
    let adapter = re_renderer::device_caps::select_testing_adapter(&instance);

    let is_ci = std::env::var("CI").is_ok();
    if is_ci {
        assert!(
            adapter.get_info().device_type == wgpu::DeviceType::Cpu,
            "We require a software renderer for CI tests. GPU based ones have been unreliable in the past, see https://github.com/rerun-io/rerun/issues/11359."
        );
    }

    let device_caps = re_renderer::device_caps::DeviceCaps::from_adapter(&adapter)
        .expect("Failed to determine device capabilities");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_caps.device_descriptor()))
            .expect("Failed to request device.");

    SharedWgpuResources {
        instance,
        adapter,
        device,
        queue,
    }
}

impl TestContext {
    /// Used to get a context with helper functions to write & read from blueprints.
    pub fn with_blueprint_ctx<R>(&self, f: impl FnOnce(TestBlueprintCtx<'_>, &StoreHub) -> R) -> R {
        let store_hub = self
            .store_hub
            .try_lock()
            .expect("Failed to get lock for blueprint ctx");

        f(
            TestBlueprintCtx {
                command_sender: &self.command_sender,
                current_blueprint: store_hub
                    .active_blueprint()
                    .expect("The test context should always have an active blueprint"),
                default_blueprint: store_hub.default_blueprint_for_app(
                    store_hub
                        .active_app()
                        .expect("The test context should always have an active app"),
                ),
                blueprint_query: &self.blueprint_query,
            },
            &store_hub,
        )
    }

    /// Helper function to send a [`SystemCommand::TimeControlCommands`] command
    /// with the current store id.
    pub fn send_time_commands(
        &self,
        store_id: StoreId,
        commands: impl IntoIterator<Item = TimeControlCommand>,
    ) {
        let commands: Vec<_> = commands.into_iter().collect();

        if !commands.is_empty() {
            self.command_sender
                .send_system(SystemCommand::TimeControlCommands {
                    store_id,
                    time_commands: commands,
                });
        }
    }

    /// Set up for rendering UI, with not 3D/2D in it.
    pub fn setup_kittest_for_rendering_ui(
        &self,
        size: impl Into<egui::Vec2>,
    ) -> egui_kittest::HarnessBuilder<()> {
        self.setup_kittest_for_rendering(re_ui::testing::TestOptions::Gui, size.into())
    }

    /// Set up for rendering 3D/2D and maybe UI.
    ///
    /// This has slightly higher error tolerances than [`Self::setup_kittest_for_rendering_ui`].
    pub fn setup_kittest_for_rendering_3d(
        &self,
        size: impl Into<egui::Vec2>,
    ) -> egui_kittest::HarnessBuilder<()> {
        self.setup_kittest_for_rendering(re_ui::testing::TestOptions::Rendering3D, size.into())
    }

    fn setup_kittest_for_rendering(
        &self,
        option: re_ui::testing::TestOptions,
        size: egui::Vec2,
    ) -> egui_kittest::HarnessBuilder<()> {
        // Egui kittests insists on having a fresh render state for each test.
        let new_render_state = create_egui_renderstate();
        let builder = re_ui::testing::new_harness(option, size).renderer(
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
    pub fn active_timeline(&self) -> Option<re_chunk::Timeline> {
        self.time_ctrl.read().timeline().copied()
    }

    pub fn active_blueprint(&mut self) -> &mut EntityDb {
        let store_hub = self.store_hub.get_mut();
        let blueprint_id = store_hub
            .active_blueprint_id()
            .expect("expected an active blueprint")
            .clone();
        store_hub.entity_db_mut(&blueprint_id)
    }

    pub fn active_store_id(&self) -> StoreId {
        self.store_hub
            .lock()
            .active_recording()
            .expect("expected an active recording")
            .store_id()
            .clone()
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
        entity_path: impl Into<EntityPath>,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    ) {
        let builder = build_chunk(Chunk::builder(entity_path));
        let store_hub = self.store_hub.get_mut();
        let active_recording = store_hub.active_recording_mut().unwrap();
        active_recording
            .add_chunk(&Arc::new(
                builder.build().expect("chunk should be successfully built"),
            ))
            .expect("chunk should be successfully added");
    }

    pub fn add_chunks(&mut self, chunks: impl Iterator<Item = Chunk>) {
        let store_hub = self.store_hub.get_mut();
        let active_recording = store_hub.active_recording_mut().unwrap();
        for chunk in chunks {
            active_recording.add_chunk(&Arc::new(chunk)).unwrap();
        }
    }

    pub fn add_rrd_manifest(&mut self, rrd_manifest: re_log_encoding::RrdManifest) {
        let store_hub = self.store_hub.get_mut();
        let active_recording = store_hub.active_recording_mut().unwrap();
        active_recording.add_rrd_manifest_message(rrd_manifest);
    }

    /// Register a view class.
    pub fn register_view_class<T: ViewClass + Default + 'static>(&mut self) {
        self.view_class_registry
            .add_class::<T>(&self.app_options, &mut self.component_fallback_registry)
            .expect("registering a class should succeed");
    }

    /// Run the provided closure with a [`ViewerContext`] produced by the [`Self`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    pub fn run(&self, egui_ctx: &egui::Context, func: impl FnOnce(&ViewerContext<'_>)) {
        re_log::PanicOnWarnScope::new(); // TODO(andreas): There should be a way to opt-out of this.
        re_ui::apply_style_and_install_loaders(egui_ctx);

        let mut store_hub = self.store_hub.lock();
        store_hub.begin_frame_caches();
        let (storage_context, store_context) = store_hub.read_context();
        let store_context = store_context
            .expect("TestContext should always have enough information to provide a store context");

        let indicated_entities_per_visualizer = self
            .view_class_registry
            .indicated_entities_per_visualizer(store_context.recording.store_id());
        let visualizable_entities_per_visualizer = self
            .view_class_registry
            .visualizable_entities_for_visualizer_systems(store_context.recording.store_id());

        let drag_and_drop_manager =
            re_viewer_context::DragAndDropManager::new(ItemCollection::default());

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
                is_test: true,

                memory_limit: re_memory::MemoryLimit::UNLIMITED,

                app_options: &self.app_options,
                reflection: &self.reflection,

                egui_ctx,
                command_sender: &self.command_sender,
                render_ctx,

                connection_registry: &self.connection_registry,
                display_mode: &DisplayMode::LocalRecordings(
                    store_context.recording_store_id().clone(),
                ),

                auth_context: None,
            },
            component_ui_registry: &self.component_ui_registry,
            component_fallback_registry: &self.component_fallback_registry,
            view_class_registry: &self.view_class_registry,
            connected_receivers: &Default::default(),
            store_context: &store_context,
            storage_context: &storage_context,
            visualizable_entities_per_visualizer: &visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &self.query_results,
            time_ctrl: &self.time_ctrl.read(),
            blueprint_time_ctrl: &Default::default(),
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
    /// Prefer not using this in conjunction with `egui_kittest`'s harness and use
    /// `egui_kittest::Harness::build_ui` instead, calling [`Self::run_ui`] inside the closure.
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

    /// Run the provided closure with a [`ViewerContext`] produced by the [`Self`] inside an existing [`egui::Ui`].
    ///
    /// IMPORTANT: call [`Self::handle_system_commands`] after calling this function if your test
    /// relies on system commands.
    pub fn run_ui(&self, ui: &mut egui::Ui, func: impl FnOnce(&ViewerContext<'_>, &mut egui::Ui)) {
        self.run(&ui.ctx().clone(), |ctx| {
            func(ctx, ui);
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

    /// Applies a fragment.
    ///
    /// Does *not* switch the active recording.
    fn go_to_dataset_data(&self, store_id: StoreId, fragment: re_uri::Fragment) {
        let re_uri::Fragment {
            selection,
            when,
            time_selection,
        } = fragment;

        if let Some(selection) = selection {
            let re_log_types::DataPath {
                entity_path,
                instance,
                component,
            } = selection;

            let item = if let Some(component) = component {
                Item::from(re_log_types::ComponentPath::new(entity_path, component))
            } else if let Some(instance) = instance {
                Item::from(InstancePath::instance(entity_path, instance))
            } else {
                Item::from(entity_path)
            };

            self.command_sender
                .send_system(SystemCommand::set_selection(item.clone()));
        }

        let mut time_commands = Vec::new();
        if let Some(time_selection) = time_selection {
            time_commands.push(TimeControlCommand::SetActiveTimeline(
                *time_selection.timeline.name(),
            ));
            time_commands.push(TimeControlCommand::SetTimeSelection(time_selection.range));
        }

        if let Some((timeline, timecell)) = when {
            time_commands.push(TimeControlCommand::SetActiveTimeline(timeline));
            time_commands.push(TimeControlCommand::SetTime(timecell.value.into()));
        }

        if !time_commands.is_empty() {
            self.command_sender
                .send_system(SystemCommand::TimeControlCommands {
                    store_id,
                    time_commands,
                });
        }
    }

    /// Best-effort attempt to meaningfully handle some of the system commands.
    pub fn handle_system_commands(&self, egui_ctx: &egui::Context) {
        while let Some((_from_where, command)) = self.command_receiver.recv_system() {
            let mut handled = true;
            let command_name = format!("{command:?}");
            match command {
                SystemCommand::SetUrlFragment { store_id, fragment } => {
                    // This adds new system commands, which will be handled later in the loop.
                    self.go_to_dataset_data(store_id, fragment);
                }
                SystemCommand::CopyViewerUrl(_) => {
                    // Ignore this trying to copy to the clipboard.
                }
                SystemCommand::AppendToStore(store_id, chunks) => {
                    let mut store_hub = self
                        .store_hub
                        .try_lock()
                        .expect("Failed to lock store hub mutex");
                    let db = store_hub.entity_db_mut(&store_id);

                    for chunk in chunks {
                        db.add_chunk(&Arc::new(chunk))
                            .expect("Updating the chunk store failed");
                    }
                }

                SystemCommand::DropEntity(store_id, entity_path) => {
                    let mut store_hub = self
                        .store_hub
                        .try_lock()
                        .expect("Failed to lock store hub mutex");
                    assert_eq!(Some(&store_id), store_hub.active_blueprint_id());

                    store_hub
                        .entity_db_mut(&store_id)
                        .drop_entity_path_recursive(&entity_path);
                }

                SystemCommand::SetSelection(set) => {
                    self.selection_state.lock().set_selection(set);
                }

                SystemCommand::SetFocus(item) => {
                    *self.focused_item.lock() = Some(item);
                }

                SystemCommand::TimeControlCommands {
                    store_id,
                    time_commands,
                } => {
                    self.with_blueprint_ctx(|blueprint_ctx, hub| {
                        let mut time_ctrl = self.time_ctrl.write();
                        let timeline_histograms = hub
                            .store_bundle()
                            .get(&store_id)
                            .expect("Invalid store id in `SystemCommand::TimeControlCommands`")
                            .timeline_histograms();

                        let blueprint_ctx =
                            Some(&blueprint_ctx).filter(|_| store_id.is_recording());

                        // We can ignore the response in the test context.
                        let res = time_ctrl.handle_time_commands(
                            blueprint_ctx,
                            timeline_histograms,
                            &time_commands,
                        );

                        if res.needs_repaint == NeedsRepaint::Yes {
                            egui_ctx.request_repaint();
                        }
                    });
                }

                // not implemented
                SystemCommand::ActivateApp(_)
                | SystemCommand::ActivateRecordingOrTable(_)
                | SystemCommand::CloseApp(_)
                | SystemCommand::CloseRecordingOrTable(_)
                | SystemCommand::LoadDataSource(_)
                | SystemCommand::AddReceiver { .. }
                | SystemCommand::ResetViewer
                | SystemCommand::ChangeDisplayMode(_)
                | SystemCommand::OpenSettings
                | SystemCommand::OpenChunkStoreBrowser
                | SystemCommand::ResetDisplayMode
                | SystemCommand::ClearActiveBlueprint
                | SystemCommand::ClearActiveBlueprintAndEnableHeuristics
                | SystemCommand::AddRedapServer { .. }
                | SystemCommand::EditRedapServerModal { .. }
                | SystemCommand::UndoBlueprint { .. }
                | SystemCommand::RedoBlueprint { .. }
                | SystemCommand::CloseAllEntries
                | SystemCommand::SetAuthCredentials { .. }
                | SystemCommand::OnAuthChanged(_)
                | SystemCommand::Logout
                | SystemCommand::SaveScreenshot { .. }
                | SystemCommand::ShowNotification { .. } => handled = false,

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

    pub fn test_help_view(help: impl Fn(OperatingSystem) -> Help) {
        use egui::os::OperatingSystem;
        let mut snapshot_results = egui_kittest::SnapshotResults::new();
        for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
            let mut harness = egui_kittest::Harness::builder().build_ui(|ui| {
                ui.ctx().set_os(os);
                re_ui::apply_style_and_install_loaders(ui.ctx());
                help(os).ui(ui);
            });
            let help_view = help(os);
            let name = format!(
                "help_view_{}_{os:?}",
                help_view
                    .title()
                    .expect("View help texts should have titles")
            )
            .replace(' ', "_")
            .to_lowercase();
            harness.fit_contents();
            harness.snapshot(&name);

            snapshot_results.extend_harness(&mut harness);
        }
    }

    /// Helper function to save the active recording to file for troubleshooting.
    ///
    /// Note: Right now it _only_ saves the recording and blueprints are ignored.
    pub fn save_recording_to_file(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let mut file = std::fs::File::create(path)?;

        let store_hub = self.store_hub.lock();
        let Some(recording_entity_db) = store_hub.active_recording() else {
            anyhow::bail!("no active recording");
        };
        let messages = recording_entity_db.to_messages(None);

        let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
        re_log_encoding::Encoder::encode_into(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            messages,
            &mut file,
        )?;

        Ok(())
    }

    /// Helper function to save the active blueprint to file for troubleshooting.
    pub fn save_blueprint_to_file(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let mut file = std::fs::File::create(path)?;

        let store_hub = self.store_hub.lock();
        let Some(blueprint_entity_db) = store_hub.active_blueprint() else {
            anyhow::bail!("no active blueprint");
        };
        let messages = blueprint_entity_db.to_messages(None);

        let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
        re_log_encoding::Encoder::encode_into(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            messages,
            &mut file,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use re_entity_db::InstancePath;
    use re_viewer_context::Item;

    use super::*;

    /// Test that `TestContext:edit_selection` works as expected, aka. its side effects are visible
    /// from `TestContext::run`.
    #[test]
    fn test_edit_selection() {
        let test_context = TestContext::new();

        let item = Item::InstancePath(InstancePath::entity_all("/entity/path"));

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
