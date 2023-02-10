//! Special error handling datastructures for debug builds (never crash!).

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

/// Tries downcasting a given value into the specified possibilities (or all of them
/// if none are specified), then run the given expression on the downcasted value.
///
/// E.g. to `dbg!()` the downcasted value on a wgpu error:
/// ```ignore
/// try_downcast!(my_error => |inner| { dbg!(inner); })
/// ```
macro_rules! try_downcast {
        ($value:expr => |$binding:pat_param| $do:expr => [$ty:ty, $($tail:ty $(,)*),*]) => {
            try_downcast!($value => |$binding| $do => $ty);
            try_downcast!($value => |$binding| $do => [$($tail),*]);
        };
        ($value:expr => |$binding:pat_param| $do:expr => [$ty:ty $(,)*]) => {
            try_downcast!($value => |$binding| $do => $ty);
        };
        ($value:expr => |$binding:pat_param| $do:expr => $ty:ty) => {
            if let Some($binding) = ($value).downcast_ref::<$ty>() {
                break Some({ $do });
            }
        };
        ($value:expr => |$binding:pat_param| $do:expr) => {
            loop {
                try_downcast![$value => |$binding| $do => [
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

/// An error with some extra deduplication logic baked in.
///
/// Implemented by default for all relevant wgpu error types, though it might be worth
/// providing a more specialized implementation for errors that are too broad by nature:
/// e.g. a shader error cannot by deduplicated simply using the shader path.
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

// Custom deduplication for shader compilation errors, based on compiler message.
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

        // try to downcast into something that implements `DedupableError`, and
        // then call `DedupableError::hash`.
        if try_downcast!(self.0.cause => |inner| DedupableError::hash(inner, state)).is_none() {
            re_log::warn!(cause=?self.0.cause, "unknown error cause");
        }
    }
}

impl PartialEq for WrappedContextError {
    fn eq(&self, rhs: &Self) -> bool {
        let mut is_eq = self.0.label.eq(&rhs.0.label)
            && self.0.label_key.eq(rhs.0.label_key)
            && self.0.string.eq(rhs.0.string);

        // try to downcast into something that implements `DedupableError`, and
        // then call `DedupableError::eq`.
        if let Some(finer_eq) =
            try_downcast!(self.0.cause => |inner| DedupableError::eq(inner, &*rhs.0.cause))
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
/// Used to avoid spamming the user with repeating errors while a pipeline
/// is in a poisoned state.
pub struct ErrorTracker {
    tick_nr: AtomicUsize,

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
            tick_nr: AtomicUsize::new(0),
            clear_countdown: AtomicI64::new(i64::MAX),
            errors: Default::default(),
        }
    }
}

impl ErrorTracker {
    /// Increment tick count used in logged errors, and clear the tracker as needed.
    pub fn tick(&self) {
        self.tick_nr.fetch_add(1, Ordering::Relaxed);

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
    /// Call this when the pipeline is back into a sane state.
    pub fn clear(&self) {
        self.errors.lock().clear();
        re_log::debug!("cleared WGPU error tracker");
    }

    /// Logs a wgpu error, making sure to deduplicate them as needed.
    pub fn handle_error(&self, error: wgpu::Error) {
        // The pipeline is in a poisoned state, errors are still coming in: we won't be
        // clearing the tracker until it had at least 2 complete frame_maintenance cycles
        // without any errors (meaning the swapchain surface is stabilized).
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
                            tick_nr = self.tick_nr.load(Ordering::Relaxed),
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
