use type_map::concurrent::{self, TypeMap};

use crate::{
    global_bindings::GlobalBindings, renderer::Renderer, resource_pools::WgpuResourcePools,
};
use crate::{FileResolver, FileServer, FileSystem, RecommendedFileResolver};

// ---

/// Any resource involving wgpu rendering which can be re-used across different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    pub(crate) shared_renderer_data: SharedRendererData,
    pub(crate) renderers: Renderers,
    pub(crate) resource_pools: WgpuResourcePools,
    pub(crate) resolver: RecommendedFileResolver,
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
    pub(crate) err_tracker: std::sync::Arc<ErrorTracker>,

    // TODO(andreas): Add frame/lifetime statistics, shared resources (e.g. "global" uniform buffer), ??
    frame_index: u64,
}

/// Startup configuration for a [`RenderContext`]
///
/// Contains any kind of configuration that doesn't change for the entire lifetime of a [`RenderContext`].
/// (flipside, if we do want to change any of these, the [`RenderContext`] needs to be re-created)
pub struct RenderContextConfig {
    /// The color format used by the eframe output buffer.
    pub output_format_color: wgpu::TextureFormat,
}

/// Immutable data that is shared between all [`Renderer`]
pub struct SharedRendererData {
    pub(crate) config: RenderContextConfig,

    /// Global bindings, always bound to 0 bind group slot zero.
    /// [`Renderer`] are not allowed to use bind group 0 themselves!
    pub(crate) global_bindings: GlobalBindings,
}

/// Struct owning *all* [`Renderer`].
/// [`Renderer`] are created lazily and stay around indefinitely.
pub(crate) struct Renderers {
    renderers: concurrent::TypeMap,
}

impl Renderers {
    pub fn get_or_create<Fs: FileSystem, R: 'static + Renderer + Send + Sync>(
        &mut self,
        shared_data: &SharedRendererData,
        resource_pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> &R {
        self.renderers
            .entry()
            .or_insert_with(|| R::create_renderer(shared_data, resource_pools, device, resolver))
    }

    pub fn get<R: 'static + Renderer>(&self) -> Option<&R> {
        self.renderers.get::<R>()
    }
}

impl RenderContext {
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, config: RenderContextConfig) -> Self {
        let mut resource_pools = WgpuResourcePools::default();
        let global_bindings = GlobalBindings::new(&mut resource_pools, device);

        // In debug builds, make sure to catch all errors, never crash, and try to
        // always let the user find a way to returned a poisoned pipeline into a sane state.
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        let err_tracker = {
            let err_tracker = std::sync::Arc::new(ErrorTracker::default());
            device.on_uncaptured_error({
                let err_tracker = std::sync::Arc::clone(&err_tracker);
                move |err| err_tracker.handle_error(err)
            });
            err_tracker
        };

        RenderContext {
            shared_renderer_data: SharedRendererData {
                config,
                global_bindings,
            },

            renderers: Renderers {
                renderers: TypeMap::new(),
            },
            resource_pools,

            resolver: crate::new_recommended_file_resolver(),

            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
            err_tracker,

            frame_index: 0,
        }
    }

    pub fn frame_maintenance(&mut self, device: &wgpu::Device) {
        // Tick the error tracker so that it knows when to reset!
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        self.err_tracker.tick();

        // Clear the resolver cache before we start reloading shaders!
        self.resolver.clear();

        // The set of files on disk that were modified in any way since last frame,
        // ignoring deletions.
        // Always an empty set in release builds.
        let modified_paths = FileServer::get_mut(|fs| fs.collect(&mut self.resolver));
        if !modified_paths.is_empty() {
            re_log::debug!(?modified_paths, "got some filesystem events");
        }

        {
            let WgpuResourcePools {
                bind_group_layouts: _,
                bind_groups,
                pipeline_layouts,
                render_pipelines,
                samplers,
                shader_modules,
                textures,
                buffers,
            } = &mut self.resource_pools; // not all pools require maintenance

            // Render pipeline maintenance must come before shader module maintenance since
            // it registers them.
            render_pipelines.frame_maintenance(
                device,
                self.frame_index,
                shader_modules,
                pipeline_layouts,
            );

            shader_modules.frame_maintenance(
                device,
                &mut self.resolver,
                self.frame_index,
                &modified_paths,
            );

            // Bind group maintenance must come before texture/buffer maintenance since it
            // registers texture/buffer use
            bind_groups.frame_maintenance(self.frame_index, textures, buffers, samplers);

            textures.frame_maintenance(self.frame_index);
            buffers.frame_maintenance(self.frame_index);
        }

        self.frame_index += 1;
    }
}

// ---

// Special error handling datastructures for debug builds (never crash!)

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
use error_handling::*;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
mod error_handling {
    use ahash::HashSet;
    use parking_lot::Mutex;
    use std::{
        hash::Hash,
        sync::{
            atomic::Ordering,
            atomic::{AtomicI64, AtomicUsize},
        },
    };
    use wgpu_core::error::ContextError;

    // ---

    /// E.g. to `dbg!()` the downcasted value on a wgpu error:
    /// ```ignore
    /// try_downcast!(|inner| { dbg!(inner); } => my_error)
    /// ```
    macro_rules! try_downcast {
        ($do:expr => $value:expr => [$ty:ty, $($tail:ty $(,),*)*]) => {
            try_downcast!($do => $value => $ty);
            try_downcast!($do => $value => [$($tail),*]);
        };
        ($do:expr => $value:expr => [$ty:ty $(,),*]) => {
            try_downcast!($do => $value => $ty);
        };
        ($do:expr => $value:expr => $ty:ty) => {
            if let Some(inner) = ($value).downcast_ref::<$ty>() {
                #[allow(clippy::redundant_closure_call)]
                break Some(($do)(inner));
            }
        };
        ($do:expr => $value:expr) => {
            loop {
                try_downcast![$do => $value => [
                    wgpu_core::command::ClearError,
                    wgpu_core::command::CommandEncoderError,
                    wgpu_core::command::ComputePassError,
                    wgpu_core::command::CopyError,
                    wgpu_core::command::DispatchError,
                    wgpu_core::command::DrawError,
                    wgpu_core::command::ExecutionError,
                    wgpu_core::command::PassErrorScope,
                    wgpu_core::command::QueryError,
                    wgpu_core::command::QueryUseError,
                    wgpu_core::command::RenderBundleError,
                    wgpu_core::command::RenderCommandError,
                    wgpu_core::command::RenderPassError,
                    wgpu_core::command::ResolveError,
                    wgpu_core::command::TransferError,
                    wgpu_core::binding_model::BindError,
                    wgpu_core::binding_model::BindingTypeMaxCountError,
                    wgpu_core::binding_model::CreateBindGroupError,
                    wgpu_core::binding_model::CreatePipelineLayoutError,
                    wgpu_core::binding_model::GetBindGroupLayoutError,
                    wgpu_core::binding_model::PushConstantUploadError,
                    wgpu_core::device::CreateDeviceError,
                    wgpu_core::device::DeviceError,
                    wgpu_core::device::RenderPassCompatibilityError,
                    wgpu_core::pipeline::ColorStateError,
                    wgpu_core::pipeline::CreateComputePipelineError,
                    wgpu_core::pipeline::CreateRenderPipelineError,
                    wgpu_core::pipeline::CreateShaderModuleError,
                    wgpu_core::pipeline::DepthStencilStateError,
                    wgpu_core::pipeline::ImplicitLayoutError,
            ]];

            break None;
        }};
    }

    fn type_of_var<T: 'static + ?Sized>(_: &T) -> std::any::TypeId {
        std::any::TypeId::of::<T>()
    }

    // ---

    trait DedupableError: Sized + std::error::Error + 'static {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            type_of_var(self).hash(state);
        }

        fn eq(&self, rhs: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
            rhs.downcast_ref::<Self>().is_some()
        }
    }

    /// E.g. to implement `DedupableError` for u32 + u64:
    /// ```ignore
    /// impl_trait![u32, u64];
    /// ```
    macro_rules! impl_trait {
        [$ty:ty, $($rest:ty),+ $(,)*] => {
            impl_trait![$ty];
            impl_trait![$($rest),+];
        };
        [$ty:ty $(,)*] => {
            impl DedupableError for $ty {}
        };
    }

    impl_trait![
        wgpu_core::command::ClearError,
        wgpu_core::command::CommandEncoderError,
        wgpu_core::command::ComputePassError,
        wgpu_core::command::CopyError,
        wgpu_core::command::DispatchError,
        wgpu_core::command::DrawError,
        wgpu_core::command::ExecutionError,
        wgpu_core::command::PassErrorScope,
        wgpu_core::command::QueryError,
        wgpu_core::command::QueryUseError,
        wgpu_core::command::RenderBundleError,
        wgpu_core::command::RenderCommandError,
        wgpu_core::command::RenderPassError,
        wgpu_core::command::ResolveError,
        wgpu_core::command::TransferError,
        wgpu_core::binding_model::BindError,
        wgpu_core::binding_model::BindingTypeMaxCountError,
        wgpu_core::binding_model::CreateBindGroupError,
        wgpu_core::binding_model::CreatePipelineLayoutError,
        wgpu_core::binding_model::GetBindGroupLayoutError,
        wgpu_core::binding_model::PushConstantUploadError,
        wgpu_core::device::CreateDeviceError,
        wgpu_core::device::DeviceError,
        wgpu_core::device::RenderPassCompatibilityError,
        wgpu_core::pipeline::ColorStateError,
        wgpu_core::pipeline::CreateComputePipelineError,
        wgpu_core::pipeline::CreateRenderPipelineError,
        // wgpu_core::pipeline::CreateShaderModuleError, // NOTE: custom impl!
        wgpu_core::pipeline::DepthStencilStateError,
        wgpu_core::pipeline::ImplicitLayoutError,
    ];

    // Custom deduplication for shader compilation errors.
    impl DedupableError for wgpu_core::pipeline::CreateShaderModuleError {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            type_of_var(self).hash(state);
            #[allow(clippy::enum_glob_use)]
            use wgpu_core::pipeline::CreateShaderModuleError::*;
            match self {
                Parsing(err) => err.source.hash(state),
                Validation(err) => err.source.hash(state),
                _ => {}
            }
        }

        fn eq(&self, rhs: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
            if rhs.downcast_ref::<Self>().is_none() {
                return false;
            }
            let rhs = rhs.downcast_ref::<Self>().unwrap();

            #[allow(clippy::enum_glob_use)]
            use wgpu_core::pipeline::CreateShaderModuleError::*;
            match (self, rhs) {
                (Parsing(err1), Parsing(err2)) => err1.source == err2.source,
                (Validation(err1), Validation(err2)) => err1.source == err2.source,
                _ => true,
            }
        }
    }

    // ---

    /// A `wgpu_core::ContextError` with hashing and equality capabilities.
    ///
    /// Used for deduplication purposes.
    #[derive(Debug)]
    pub struct WrappedContextError(Box<ContextError>);
    impl std::hash::Hash for WrappedContextError {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.label.hash(state); // e.g. "composite_encoder"
            self.0.label_key.hash(state); // e.g. "encoder"
            self.0.string.hash(state); // e.g. "a RenderPass"

            // try to downcast into something that implements `FinerGrainedDedup`, and
            // then call `FinerGrainedDedup::hash`.
            if try_downcast!(|inner| DedupableError::hash(inner, state) => self.0.cause).is_none() {
                re_log::warn!(cause=?self.0.cause, "unknown error cause");
            }
        }
    }
    impl PartialEq for WrappedContextError {
        fn eq(&self, rhs: &Self) -> bool {
            let mut is_eq = self.0.label.eq(&rhs.0.label)
                && self.0.label_key.eq(rhs.0.label_key)
                && self.0.string.eq(rhs.0.string);

            // try to downcast into something that implements `FinerGrainedDedup`, and
            // then call `FinerGrainedDedup::eq`.
            if let Some(finer_eq) =
                try_downcast!(|inner| DedupableError::eq(inner, &*rhs.0.cause) => self.0.cause)
            {
                is_eq |= finer_eq;
            } else {
                re_log::warn!(cause=?self.0.cause, "unknown error cause");
            }

            is_eq
        }
    }
    impl Eq for WrappedContextError {}

    // ---

    /// Coalesces wgpu errors until the tracker is `clear()`ed.
    ///
    /// Used to avoid spamming the user with repeating errors while the pipeline
    /// is in a poisoned state.
    pub struct ErrorTracker {
        frame_nr: AtomicUsize,
        /// This countdown reaching 0 indicates that the pipeline has stabilized into a
        /// sane state, which might take a few frames if we've just left a poisoned state.
        ///
        /// We use this to know when it makes sense to clear the error tracker.
        clear_countdown: AtomicI64,
        errors: Mutex<HashSet<WrappedContextError>>,
    }
    impl Default for ErrorTracker {
        fn default() -> Self {
            Self {
                frame_nr: AtomicUsize::new(0),
                clear_countdown: AtomicI64::new(i64::MAX),
                errors: Default::default(),
            }
        }
    }
    impl ErrorTracker {
        /// Increment frame count used in logged errors.
        // TODO: update doc
        pub fn tick(&self) {
            self.frame_nr.fetch_add(1, Ordering::Relaxed);

            // The pipeline has stabilized back into a sane state, clear
            // the error tracker so that we're ready to log errors once again
            // if the pipeline gets back into a poisoned state.
            if self.clear_countdown.fetch_sub(1, Ordering::Relaxed) == 1 {
                self.clear_countdown.store(i64::MAX, Ordering::Relaxed);
                self.clear();
                re_log::info!("pipeline back into a sane state!");
            }
        }

        /// Resets the tracker.
        ///
        /// Call this when the pipeline is back to a sane state.
        pub fn clear(&self) {
            self.errors.lock().clear();
            re_log::debug!("cleared WGPU error tracker");
        }

        /// Logs a wgpu error, making sure to deduplicate them as needed.
        pub fn handle_error(&self, error: wgpu::Error) {
            // The pipeline is in a poisoned state, errors are still coming in: we won't be
            // clearing the tracker until it had at least 3 error-free frames to stabilize.
            self.clear_countdown.store(3, Ordering::Relaxed);

            match error {
                wgpu::Error::OutOfMemory { source: _ } => panic!("{error}"),
                wgpu::Error::Validation {
                    source,
                    description,
                } => {
                    match source.downcast::<ContextError>() {
                        Ok(ctx_err) => {
                            if ctx_err
                                .cause
                                .downcast_ref::<wgpu_core::command::CommandEncoderError>()
                                .is_some()
                            {
                                // Actual command encoder errors never carry any meaningful
                                // information: ignore them.
                                return;
                            }

                            let ctx_err = WrappedContextError(ctx_err);
                            if !self.errors.lock().insert(ctx_err) {
                                // We've already logged this error since we've entered the
                                // current poisoned state. Don't log it again.
                                return;
                            }

                            re_log::error!(
                                frame_nr = self.frame_nr.load(Ordering::Relaxed),
                                %description,
                                "WGPU error",
                            );
                        }
                        Err(err) => panic!("{err}"),
                    };
                }
            }
        }
    }
}
