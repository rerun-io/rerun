use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use re_mutex::Mutex;
use type_map::concurrent::TypeMap;

use crate::allocator::{CpuWriteGpuReadBelt, GpuReadbackBelt};
use crate::device_caps::DeviceCaps;
use crate::error_handling::{ErrorTracker, WgpuErrorScope};
use crate::global_bindings::GlobalBindings;
use crate::renderer::{Renderer, RendererExt};
use crate::resource_managers::{TextureManager2D, TextureManager3D};
use crate::wgpu_resources::WgpuResourcePools;
use crate::{FileServer, RecommendedFileResolver};

/// Frame idx used before starting the first frame.
const STARTUP_FRAME_IDX: u64 = u64::MAX;

#[derive(thiserror::Error, Debug)]
pub enum RenderContextError {
    #[error(
        "The GPU/graphics driver is lacking some abilities: {0}. \
        Check the troubleshooting guide at https://rerun.io/docs/getting-started/troubleshooting and consider updating your graphics driver."
    )]
    InsufficientDeviceCapabilities(#[from] crate::device_caps::InsufficientDeviceCapabilities),
}

/// Controls MSAA (Multi-Sampling Anti-Aliasing)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MsaaMode {
    /// Disabled MSAA.
    ///
    /// Preferred option for testing since MSAA implementations vary across devices,
    /// especially in alpha-to-coverage cases.
    ///
    /// Note that this doesn't necessarily mean that we never use any multisampled targets,
    /// merely that the main render target is not multisampled.
    /// Some renderers/postprocessing effects may still incorporate textures with a sample count higher than 1.
    Off,

    /// 4x MSAA.
    ///
    /// As of writing 4 samples is the only option (other than _Off_) that works with `WebGPU`,
    /// and it is guaranteed to be always available.
    // TODO(andreas): On native we could offer higher counts.
    #[default]
    Msaa4x,
}

impl MsaaMode {
    /// Returns the number of samples for this MSAA mode.
    pub const fn sample_count(&self) -> u32 {
        match self {
            Self::Off => 1,
            Self::Msaa4x => 4,
        }
    }
}

/// Configures global properties of the renderer.
///
/// For simplicity, we don't allow changing any of these properties without tearing down the [`RenderContext`],
/// even though it may be possible.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderConfig {
    pub msaa_mode: MsaaMode,
    // TODO(andreas): Add a way to force the render tier?
}

impl RenderConfig {
    /// Returns the best config for the given [`DeviceCaps`].
    pub fn best_for_device_caps(_device_caps: &DeviceCaps) -> Self {
        Self {
            msaa_mode: MsaaMode::Msaa4x,
        }
    }

    /// Render config preferred for running most tests.
    ///
    /// This is optimized for low discrepancy between devices in order to
    /// to keep image comparison thresholds low.
    pub fn testing() -> Self {
        Self {
            // we use "testing" also for generating nice looking screenshots
            msaa_mode: MsaaMode::Msaa4x,
        }
    }
}

/// Any resource involving wgpu rendering which can be re-used across different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    device_caps: DeviceCaps,
    config: RenderConfig,
    output_format_color: wgpu::TextureFormat,

    /// Global bindings, always bound to 0 bind group slot zero.
    /// [`Renderer`] are not allowed to use bind group 0 themselves!
    pub global_bindings: GlobalBindings,

    renderers: RwLock<Renderers>,
    pub(crate) resolver: RecommendedFileResolver,

    pub texture_manager_2d: TextureManager2D,
    pub texture_manager_3d: TextureManager3D,
    pub cpu_write_gpu_read_belt: Mutex<CpuWriteGpuReadBelt>,
    pub gpu_readback_belt: Mutex<GpuReadbackBelt>,

    /// List of unfinished queue submission via this context.
    ///
    /// This is currently only about submissions we do via the global encoder in [`ActiveFrameContext`]
    /// TODO(andreas): We rely on egui for the "primary" submissions in `re_viewer`. It would be nice to take full control over all submissions.
    inflight_queue_submissions: Vec<wgpu::SubmissionIndex>,

    pub active_frame: ActiveFrameContext,

    /// Frame index used for [`wgpu::Device::on_uncaptured_error`] callbacks.
    ///
    /// Today, when using wgpu-core (== native & webgl) this is equal to the current [`ActiveFrameContext::frame_index`]
    /// since the content timeline is in sync with the device timeline,
    /// meaning everything done on [`wgpu::Device`] happens right away.
    /// On WebGPU however, the `content timeline` may be arbitrarily behind the `device timeline`!
    /// See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
    frame_index_for_uncaptured_errors: Arc<AtomicU64>,

    /// Error tracker used for `top_level_error_scope` and [`wgpu::Device::on_uncaptured_error`].
    top_level_error_tracker: Arc<ErrorTracker>,

    pub gpu_resources: WgpuResourcePools, // Last due to drop order.
}

/// Struct owning *all* [`Renderer`].
/// [`Renderer`] are created lazily and stay around indefinitely.
#[derive(Default)]
pub struct Renderers {
    renderers: TypeMap,
    renderers_by_key: Vec<Arc<dyn RendererExt>>,
}

/// Unique identifier for a [`Renderer`] type.
///
/// We generally don't expect many different distinct types of renderers,
/// therefore 255 should be more than enough.
/// This limitation simplifies sorting of drawables a bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RendererTypeId(u8);

impl RendererTypeId {
    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

pub struct RendererWithKey<T: Renderer> {
    renderer: Arc<T>,
    key: RendererTypeId,
}

impl<T: Renderer> std::ops::Deref for RendererWithKey<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.renderer.as_ref()
    }
}

impl<T: Renderer> RendererWithKey<T> {
    /// Returns the key of the renderer.
    ///
    /// The key is guaranteed to be unique and constant for the lifetime of the renderer.
    /// It is kept as small as possible to aid with sorting drawables.
    #[inline]
    pub fn key(&self) -> RendererTypeId {
        self.key
    }
}

impl Renderers {
    pub fn get_or_create<R: 'static + Renderer + Send + Sync>(
        &mut self,
        ctx: &RenderContext,
    ) -> &RendererWithKey<R> {
        self.renderers.entry().or_insert_with(|| {
            re_tracing::profile_scope!("create_renderer", std::any::type_name::<R>());

            let key = RendererTypeId(u8::try_from(self.renderers_by_key.len()).unwrap_or_else(
                |_| {
                    re_log::error!("Supporting at most {} distinct renderer types.", u8::MAX);
                    u8::MAX
                },
            ));

            let renderer = Arc::new(R::create_renderer(ctx));
            self.renderers_by_key.push(renderer.clone());

            RendererWithKey { renderer, key }
        })
    }

    pub fn get<R: 'static + Renderer>(&self) -> Option<&RendererWithKey<R>> {
        self.renderers.get::<RendererWithKey<R>>()
    }

    /// Gets a renderer by its key.
    ///
    /// For this to succeed, the renderer must have been initialized prior.
    /// (there would be no key otherwise anyways!)
    /// The returned type is the type erased [`RendererExt`] rather than a concrete renderer type.
    pub(crate) fn get_by_key(&self, key: RendererTypeId) -> Option<&dyn RendererExt> {
        self.renderers_by_key
            .get(key.0 as usize)
            .map(|r| r.as_ref())
    }
}

impl RenderContext {
    /// Chunk size for our cpu->gpu buffer manager.
    ///
    /// 32MiB chunk size (as big as a for instance a 2048x1024 float4 texture)
    /// (it's tempting to use something smaller on Web, but this may just cause more
    /// buffers to be allocated the moment we want to upload a bigger chunk)
    const CPU_WRITE_GPU_READ_BELT_DEFAULT_CHUNK_SIZE: Option<wgpu::BufferSize> =
        wgpu::BufferSize::new(1024 * 1024 * 32);

    /// Chunk size for our gpu->cpu buffer manager.
    ///
    /// We expect large screenshots to be rare occurrences, so we go with fairly small chunks of just 64 kiB.
    /// (this is as much memory as a 128x128 rgba8 texture, or a little bit less than a 64x64 picking target with depth)
    /// I.e. screenshots will end up in dedicated chunks.
    const GPU_READBACK_BELT_DEFAULT_CHUNK_SIZE: Option<wgpu::BufferSize> =
        wgpu::BufferSize::new(1024 * 64);

    /// Limit maximum number of in flight submissions to this number.
    ///
    /// By limiting the number of submissions we have on the queue we ensure that GPU stalls do not
    /// cause us to request more and more memory to prepare more and more submissions.
    ///
    /// Note that this is only indirectly related to number of buffered frames,
    /// since buffered frames/blit strategy are all about the display<->gpu interface,
    /// whereas this is about a *particular aspect* of the cpu<->gpu interface.
    ///
    /// Should be somewhere between 1-4, too high and we use up more memory and introduce latency,
    /// too low and we may starve the GPU.
    const MAX_NUM_INFLIGHT_QUEUE_SUBMISSIONS: usize = 4;

    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        output_format_color: wgpu::TextureFormat,
        config_provider: impl FnOnce(&DeviceCaps) -> RenderConfig,
    ) -> Result<Self, RenderContextError> {
        re_tracing::profile_function!();

        #[cfg(not(load_shaders_from_disk))]
        {
            // Make sure `workspace_shaders::init()` is called at least once, which will
            // register all shaders defined in the workspace into the run-time in-memory
            // filesystem.
            crate::workspace_shaders::init();
        }

        let device_caps = DeviceCaps::from_adapter(adapter)?;
        let config = config_provider(&device_caps);

        let frame_index_for_uncaptured_errors = Arc::new(AtomicU64::new(STARTUP_FRAME_IDX));

        // Make sure to catch all errors, never crash, and deduplicate reported errors.
        // `on_uncaptured_error` is a last-resort handler which we should never hit,
        // since there should always be an open error scope.
        //
        // Note that this handler may not be called for all errors!
        // (as of writing, wgpu-core will always call it when there's no open error scope, but Dawn doesn't!)
        // Therefore, it is important to always have a `WgpuErrorScope` open!
        // See https://www.w3.org/TR/webgpu/#telemetry
        let top_level_error_tracker = {
            let err_tracker = Arc::new(ErrorTracker::default());
            device.on_uncaptured_error({
                let err_tracker = Arc::clone(&err_tracker);
                let frame_index_for_uncaptured_errors = frame_index_for_uncaptured_errors.clone();
                Arc::new(move |err| {
                    err_tracker.handle_error(
                        err,
                        frame_index_for_uncaptured_errors.load(Ordering::Acquire),
                    );
                })
            });
            err_tracker
        };
        let top_level_error_scope = Some(WgpuErrorScope::start(&device));

        log_adapter_info(&adapter.get_info());

        let mut gpu_resources = WgpuResourcePools::default();
        let global_bindings = GlobalBindings::new(&gpu_resources, &device);

        let resolver = crate::new_recommended_file_resolver();
        let texture_manager_2d = TextureManager2D::new(&device, &queue, &gpu_resources.textures);
        let texture_manager_3d = TextureManager3D::new();

        let active_frame = ActiveFrameContext {
            before_view_builder_encoder: Mutex::new(FrameGlobalCommandEncoder::new(&device)),
            frame_index: STARTUP_FRAME_IDX,
            top_level_error_scope,
            num_view_builders_created: AtomicU64::new(0),
        };

        // Register shader workarounds for the current device.
        if adapter.get_info().backend == wgpu::Backend::BrowserWebGpu {
            // Chrome/Tint does not support `@invariant` when targeting Metal.
            // https://bugs.chromium.org/p/chromium/issues/detail?id=1439273
            // (bug is fixed as of writing, but hasn't hit any public released version yet)
            // Ignoring it is fine in the cases we use it, it's mostly there to avoid a (correct!) warning in wgpu.
            gpu_resources
                .shader_modules
                .shader_text_workaround_replacements
                .push((
                    "@invariant @builtin(position)".to_owned(),
                    "@builtin(position)".to_owned(),
                ));
        }

        let cpu_write_gpu_read_belt = Mutex::new(CpuWriteGpuReadBelt::new(
            Self::CPU_WRITE_GPU_READ_BELT_DEFAULT_CHUNK_SIZE.unwrap(),
        ));
        let gpu_readback_belt = Mutex::new(GpuReadbackBelt::new(
            Self::GPU_READBACK_BELT_DEFAULT_CHUNK_SIZE.unwrap(),
        ));

        Ok(Self {
            device,
            queue,
            device_caps,
            config,
            output_format_color,
            global_bindings,
            renderers: RwLock::new(Renderers::default()),
            resolver,
            top_level_error_tracker,
            texture_manager_2d,
            texture_manager_3d,
            cpu_write_gpu_read_belt,
            gpu_readback_belt,
            inflight_queue_submissions: Vec::new(),
            active_frame,
            frame_index_for_uncaptured_errors,
            gpu_resources,
        })
    }

    fn poll_device(&mut self) {
        re_tracing::profile_function!();

        // Ensure not too many queue submissions are in flight.

        let num_submissions_to_wait_for = self
            .inflight_queue_submissions
            .len()
            .saturating_sub(Self::MAX_NUM_INFLIGHT_QUEUE_SUBMISSIONS);

        if let Some(newest_submission_to_wait_for) = self
            .inflight_queue_submissions
            .drain(0..num_submissions_to_wait_for)
            .next_back()
        {
            // Disable error reporting on Web:
            // * On WebGPU poll is a no-op and we don't get here.
            // * On WebGL we'll just immediately timeout since we can't actually wait for frames.
            if !cfg!(target_arch = "wasm32")
                && let Err(err) = self.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(newest_submission_to_wait_for),
                    timeout: None,
                })
            {
                re_log::warn_once!(
                    "Failed to limit number of in-flight GPU frames to {}: {:?}",
                    Self::MAX_NUM_INFLIGHT_QUEUE_SUBMISSIONS,
                    err
                );
            }
        }
    }

    /// Call this at the beginning of a new frame.
    ///
    /// Updates internal book-keeping, frame allocators and executes delayed events like shader reloading.
    pub fn begin_frame(&mut self) {
        re_tracing::profile_function!();

        // If the currently active frame still has an encoder, we need to finish it and queue it.
        // This should only ever happen for the first frame where we created an encoder for preparatory work. Every other frame we take the encoder at submit!
        if self
            .active_frame
            .before_view_builder_encoder
            .lock()
            .0
            .is_some()
        {
            if self.active_frame.frame_index != STARTUP_FRAME_IDX {
                re_log::error!("There was still a command encoder from the previous frame at the beginning of the current.
This means, either a call to RenderContext::before_submit was omitted, or the previous frame was unexpectedly cancelled.");
            }
            self.before_submit();
        }

        // Request write-staging buffers back.
        // Ideally we'd do this as closely as possible to the last submission containing any cpu->gpu operations as possible.
        self.cpu_write_gpu_read_belt.get_mut().after_queue_submit();

        // Schedule mapping for all read staging buffers.
        // Ideally we'd do this as closely as possible to the last submission containing any gpu->cpu operations as possible.
        self.gpu_readback_belt.get_mut().after_queue_submit();

        // Close previous' frame error scope.
        if let Some(top_level_error_scope) = self.active_frame.top_level_error_scope.take() {
            let frame_index_for_uncaptured_errors = self.frame_index_for_uncaptured_errors.clone();
            self.top_level_error_tracker.handle_error_future(
                self.device_caps.backend_type,
                top_level_error_scope.end(),
                self.active_frame.frame_index,
                move |err_tracker, frame_index| {
                    // Update last completed frame index.
                    //
                    // Note that this means that the device timeline has now finished this frame as well!
                    // Reminder: On WebGPU the device timeline may be arbitrarily behind the content timeline!
                    // See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
                    frame_index_for_uncaptured_errors.store(frame_index, Ordering::Release);
                    err_tracker.on_device_timeline_frame_finished(frame_index);

                    // TODO(#4507): Once we support creating more error handlers,
                    // we need to tell all of them here that the frame has finished.
                },
            );
        }

        // New active frame!
        self.active_frame = ActiveFrameContext {
            before_view_builder_encoder: Mutex::new(FrameGlobalCommandEncoder::new(&self.device)),
            frame_index: self.active_frame.frame_index.wrapping_add(1),
            top_level_error_scope: Some(WgpuErrorScope::start(&self.device)),
            num_view_builders_created: AtomicU64::new(0),
        };
        let frame_index = self.active_frame.frame_index;

        // The set of files on disk that were modified in any way since last frame,
        // ignoring deletions.
        // Always an empty set in release builds.
        let modified_paths = FileServer::get_mut(|fs| fs.collect(&self.resolver));
        if !modified_paths.is_empty() {
            re_log::debug!(?modified_paths, "got some filesystem events");
        }

        self.texture_manager_2d.begin_frame(frame_index);
        self.texture_manager_3d.begin_frame();
        self.gpu_readback_belt.get_mut().begin_frame(frame_index);

        {
            let WgpuResourcePools {
                bind_group_layouts,
                bind_groups,
                pipeline_layouts,
                render_pipelines,
                samplers,
                shader_modules,
                textures,
                buffers,
            } = &mut self.gpu_resources; // not all pools require maintenance

            // Shader module maintenance must come before render pipelines because render pipeline
            // recompilation picks up all shaders that have been recompiled this frame.
            shader_modules.begin_frame(&self.device, &self.resolver, frame_index, &modified_paths);
            render_pipelines.begin_frame(
                &self.device,
                frame_index,
                shader_modules,
                pipeline_layouts,
            );

            bind_groups.begin_frame(frame_index, textures, buffers, samplers);

            textures.begin_frame(frame_index);
            buffers.begin_frame(frame_index);

            pipeline_layouts.begin_frame(frame_index);
            bind_group_layouts.begin_frame(frame_index);
            samplers.begin_frame(frame_index);
        }

        // Poll device *after* resource pool `begin_frame` since resource pools may each decide drop resources.
        // Wgpu internally may then internally decide to let go of these buffers.
        self.poll_device();
    }

    /// Call this at the end of a frame but before submitting command buffers (e.g. from [`crate::view_builder::ViewBuilder`])
    pub fn before_submit(&mut self) {
        re_tracing::profile_function!();

        // Unmap all write staging buffers, so we don't get validation errors about buffers still being mapped
        // that the gpu wants to read from.
        self.cpu_write_gpu_read_belt.lock().before_queue_submit();

        if let Some(command_encoder) = self
            .active_frame
            .before_view_builder_encoder
            .lock()
            .0
            .take()
        {
            re_tracing::profile_scope!("finish & submit frame-global encoder");
            let command_buffer = command_encoder.finish();

            // TODO(andreas): For better performance, we should try to bundle this with the single submit call that is currently happening in eframe.
            //                  How do we hook in there and make sure this buffer is submitted first?
            self.inflight_queue_submissions
                .push(self.queue.submit([command_buffer]));
        }
    }

    /// Gets a renderer with the specified type, initializing it if necessary.
    pub fn renderer<R: 'static + Renderer + Send + Sync>(
        &self,
    ) -> MappedRwLockReadGuard<'_, RendererWithKey<R>> {
        // Most likely we already have the renderer. Take a read lock and return it.
        if let Ok(renderer) =
            parking_lot::RwLockReadGuard::try_map(self.renderers.read(), |r| r.get::<R>())
        {
            return renderer;
        }

        // If it wasn't there we have to add it.
        // This path is rare since it happens only once per renderer type in the lifetime of the ctx.
        // (we don't discard renderers ever)
        self.renderers.write().get_or_create::<R>(self);

        // Release write lock again and only take a read lock.
        // safe to unwrap since we just created it and nobody removes elements from the renderer.
        parking_lot::RwLockReadGuard::map(self.renderers.read(), |r| r.get::<R>().unwrap())
    }

    /// Read access to renderers.
    pub(crate) fn read_lock_renderers(&self) -> RwLockReadGuard<'_, Renderers> {
        self.renderers.read()
    }

    /// Returns the global frame index of the active frame.
    pub fn active_frame_idx(&self) -> u64 {
        self.active_frame.frame_index
    }

    /// Returns the device's capabilities.
    pub fn device_caps(&self) -> &DeviceCaps {
        &self.device_caps
    }

    /// Returns the active render config.
    pub fn render_config(&self) -> &RenderConfig {
        &self.config
    }

    /// Returns the final output format for color (i.e. the surface's format).
    pub fn output_format_color(&self) -> wgpu::TextureFormat {
        self.output_format_color
    }
}

pub struct FrameGlobalCommandEncoder(Option<wgpu::CommandEncoder>);

impl FrameGlobalCommandEncoder {
    fn new(device: &wgpu::Device) -> Self {
        Self(Some(device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label:
                    crate::DebugLabel::from("global \"before viewbuilder\" command encoder").get(),
            },
        )))
    }

    /// Gets the global encoder for a frame. Only valid within a frame.
    pub fn get(&mut self) -> &mut wgpu::CommandEncoder {
        self.0
            .as_mut()
            .expect("Frame global encoder can't be accessed outside of a frame!")
    }
}

impl Drop for FrameGlobalCommandEncoder {
    fn drop(&mut self) {
        // Close global command encoder if there is any pending.
        // Not doing so before shutdown causes errors!
        if let Some(encoder) = self.0.take() {
            encoder.finish();
        }
    }
}

pub struct ActiveFrameContext {
    /// Command encoder for all commands that should go in before view builder are submitted.
    ///
    /// This should be used for any gpu copy operation outside of a renderer or view builder.
    /// (i.e. typically in [`crate::renderer::DrawData`] creation!)
    pub before_view_builder_encoder: Mutex<FrameGlobalCommandEncoder>,

    /// Index of this frame. Is incremented for every render frame.
    ///
    /// Keep in mind that all operations on WebGPU are asynchronous:
    /// This counter is part of the `content timeline` and may be arbitrarily
    /// behind both of the `device timeline` and `queue timeline`.
    /// See <https://www.w3.org/TR/webgpu/#programming-model-timelines>
    pub frame_index: u64,

    /// Top level device error scope, created at startup and closed & reopened on every frame.
    ///
    /// According to documentation, not all errors may be caught by [`wgpu::Device::on_uncaptured_error`].
    /// <https://www.w3.org/TR/webgpu/#eventdef-gpudevice-uncapturederror>
    /// Therefore, we should make sure that we _always_ have an error scope open!
    /// Additionally, we use this to update [`RenderContext::frame_index_for_uncaptured_errors`].
    ///
    /// The only time this is allowed to be `None` is during shutdown and when closing an old and opening a new scope.
    top_level_error_scope: Option<WgpuErrorScope>,

    /// Number of view builders created in this frame so far.
    pub num_view_builders_created: AtomicU64,
}

impl ActiveFrameContext {
    /// Returns the number of view builders created in this frame so far.
    pub fn num_view_builders_created(&self) -> u64 {
        // Uses acquire semenatics to be on the safe side (side effects from the ViewBuilder creation is visible to the caller).
        self.num_view_builders_created.load(Ordering::Acquire)
    }
}

fn log_adapter_info(info: &wgpu::AdapterInfo) {
    re_tracing::profile_function!();

    // See https://github.com/rerun-io/rerun/issues/3089
    let is_software_rasterizer_with_known_crashes = {
        // `llvmpipe` is Mesa's software rasterizer.
        // It may describe EITHER a Vulkan or OpenGL software rasterizer.
        // `lavapipe` is the name given to the Vulkan software rasterizer,
        // but this name doesn't seem to show up in the info string.
        let is_mesa_software_rasterizer = info.driver == "llvmpipe";

        // TODO(andreas):
        // Some versions of lavapipe are problematic (we observed crashes in the past),
        // but we haven't isolated for what versions this happens.
        // (we are happily using lavapipe without any issues on CI)
        // However, there's reason to be more skeptical of OpenGL software rasterizers,
        // so we mark those as problematic regardless.
        // A user might as well just use Vulkan software rasterizer if they're in a situation where they
        // can't use a GPU for which we do have test coverage.
        info.backend == wgpu::Backend::Gl && is_mesa_software_rasterizer
    };

    let human_readable_summary = adapter_info_summary(info);

    if cfg!(test) {
        // If we're testing then software rasterizers are just fine, preferred even!
        re_log::debug_once!("wgpu adapter {human_readable_summary}");
    } else if is_software_rasterizer_with_known_crashes {
        re_log::warn_once!(
            "Bad software rasterizer detected - expect poor performance and crashes. See: https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues"
        );
        re_log::info_once!("wgpu adapter {human_readable_summary}");
    } else if info.device_type == wgpu::DeviceType::Cpu {
        re_log::warn_once!(
            "Software rasterizer detected - expect poor performance. See: https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues"
        );
        re_log::info_once!("wgpu adapter {human_readable_summary}");
    } else {
        re_log::debug_once!("wgpu adapter {human_readable_summary}");
    }
}

/// A human-readable summary about an adapter
pub fn adapter_info_summary(info: &wgpu::AdapterInfo) -> String {
    let wgpu::AdapterInfo {
        name,
        vendor: _, // skip integer id
        device: _, // skip integer id
        device_type,
        driver,
        driver_info,
        backend,
    } = &info;

    // Example values:
    // > name: "llvmpipe (LLVM 16.0.6, 256 bits)", device_type: Cpu, backend: Vulkan, driver: "llvmpipe", driver_info: "Mesa 23.1.6-arch1.4 (LLVM 16.0.6)"
    // > name: "Apple M1 Pro", device_type: IntegratedGpu, backend: Metal, driver: "", driver_info: ""
    // > name: "ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)", device_type: IntegratedGpu, backend: Gl, driver: "", driver_info: ""

    let mut summary = format!("backend: {backend:?}, device_type: {device_type:?}");

    if !name.is_empty() {
        summary += &format!(", name: {name:?}");
    }
    if !driver.is_empty() {
        summary += &format!(", driver: {driver:?}");
    }
    if !driver_info.is_empty() {
        summary += &format!(", driver_info: {driver_info:?}");
    }

    summary
}
