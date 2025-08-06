mod error_tracker;
mod now_or_never;
mod wgpu_core_error;
mod wgpu_error_scope;

pub use error_tracker::ErrorTracker;
pub use wgpu_error_scope::WgpuErrorScope;

use crate::device_caps::WgpuBackendType;

// -------

fn handle_async_error(
    backend_type: WgpuBackendType,
    resolve_callback: impl FnOnce(Option<wgpu::Error>) + 'static,
    error_future: impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
) {
    match backend_type {
        #[cfg(web)]
        WgpuBackendType::WebGpu => wasm_bindgen_futures::spawn_local(async move {
            resolve_callback(error_future.await);
        }),
        WgpuBackendType::WgpuCore => {
            if let Some(error) = now_or_never::now_or_never(error_future) {
                resolve_callback(error);
            } else {
                re_log::error_once!(
                    "Expected wgpu errors to be ready immediately when using any of the wgpu-core based (native & webgl) backends."
                );
            }
        }
    }
}
