use std::sync::Arc;

/// Wgpu device error scope for all filters that auto closes when exiting the scope unless it was already closed.
///
/// The expectation is that the scope is manually closed, but this construct is useful to not accidentally
/// leave the scope open when returning early from a function.
/// Opens scopes for all error types.
pub struct WgpuErrorScope {
    open: bool,
    device: Arc<wgpu::Device>,
}

impl WgpuErrorScope {
    pub fn start(device: &Arc<wgpu::Device>) -> Self {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        device.push_error_scope(wgpu::ErrorFilter::Internal);
        Self {
            device: device.clone(),
            open: true,
        }
    }

    pub fn end(
        mut self,
    ) -> [impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static; 3] {
        self.open = false;
        [
            self.device.pop_error_scope(),
            self.device.pop_error_scope(),
            self.device.pop_error_scope(),
        ]
    }
}

impl Drop for WgpuErrorScope {
    fn drop(&mut self) {
        if self.open {
            drop(self.device.pop_error_scope());
            drop(self.device.pop_error_scope());
            drop(self.device.pop_error_scope());
        }
    }
}
