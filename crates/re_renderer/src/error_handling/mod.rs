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
    error_callback: impl FnOnce(wgpu::Error) + 'static,
    error_future: impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
) {
    #[cfg(webgpu)]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(error) = error_future.await {
                error_callback(error);
            }
        });
    }
    #[cfg(not(webgpu))]
    {
        if let Some(error_future) = now_or_never::now_or_never(error_future) {
            if let Some(error) = error_future {
                error_callback(error);
            }
        } else {
            re_log::error_once!("Expected all native wgpu errors to be ready immediately.");
        }
    }
}
