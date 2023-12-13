mod error_tracker;
mod wgpu_error_scope;

#[cfg(not(webgpu))]
mod wgpu_core_error;

#[cfg(not(webgpu))]
mod now_or_never;

pub use error_tracker::ErrorTracker;
pub use wgpu_error_scope::WgpuErrorScope;

// -------

fn handle_async_error(
    resolve_callback: impl FnOnce(Option<wgpu::Error>) + 'static,
    error_future: impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
) {
    cfg_if::cfg_if! {
        if #[cfg(webgpu)] {
            wasm_bindgen_futures::spawn_local(async move {
                resolve_callback(error_future.await);
            });
        } else {
            if let Some(error) = now_or_never::now_or_never(error_future) {
                resolve_callback(error);
            } else {
                re_log::error_once!("Expected wgpu errors to be ready immediately when using any of the wgpu-core based (native & webgl) backends.");
            }
        }
    }
}
