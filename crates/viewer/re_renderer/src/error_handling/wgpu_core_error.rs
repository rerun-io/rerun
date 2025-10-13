use std::hash::Hash as _;

use wgpu::wgc as wgpu_core;

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
        match self {
            Self::Parsing(err) => err.source.hash(state),
            Self::Validation(err) => err.source.hash(state),
            _ => {}
        }
    }

    fn eq(&self, rhs: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
        if rhs.downcast_ref::<Self>().is_none() {
            return false;
        }
        let rhs = rhs.downcast_ref::<Self>().unwrap();

        match (self, rhs) {
            (Self::Parsing(err1), Self::Parsing(err2)) => err1.source == err2.source,
            (Self::Validation(err1), Self::Validation(err2)) => err1.source == err2.source,
            _ => true,
        }
    }
}

/// A `wgpu_core::ContextError` with hashing and equality capabilities.
///
/// Used for deduplication purposes.
#[derive(Debug)]
pub struct WgpuCoreWrappedContextError(pub Box<wgpu_core::error::ContextError>);

impl std::hash::Hash for WgpuCoreWrappedContextError {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // If we haven't set a debug label ourselves, the label is typically not stable across frames,
        // Since wgc fills in the generation counter.
        // Snip that part, starting with the last occurrence of -(.
        let label = if let Some(index) = self.0.label.find("-(") {
            &self.0.label[..index]
        } else {
            &self.0.label
        };

        label.hash(state); // e.g. "composite_encoder"
        self.0.fn_ident.hash(state);

        // try to downcast into something that implements `DedupableError`, and
        // then call `DedupableError::hash`.
        if try_downcast!(self.0.source => |inner| DedupableError::hash(inner, state)).is_none() {
            re_log::warn!(cause=?self.0.source, "unknown error cause");
        }
    }
}

impl PartialEq for WgpuCoreWrappedContextError {
    fn eq(&self, rhs: &Self) -> bool {
        let mut is_eq = self.0.label.eq(&rhs.0.label) && self.0.fn_ident.eq(rhs.0.fn_ident);

        // try to downcast into something that implements `DedupableError`, and
        // then call `DedupableError::eq`.
        if let Some(finer_eq) =
            try_downcast!(self.0.source => |inner| DedupableError::eq(inner, &*rhs.0.source))
        {
            is_eq |= finer_eq;
        } else {
            re_log::warn!(cause=?self.0.source, "unknown error cause");
        }

        is_eq
    }
}

impl Eq for WgpuCoreWrappedContextError {}
