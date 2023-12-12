/// Wgpu device error scope for all filters that auto closes when exiting the scope unless it was already closed.
///
/// The expectation is that the scope is manually closed, but this construct is useful to not accidentally
/// leave the scope open when returning early from a function.
/// Opens scopes for all error types.
pub struct WgpuErrorScope<'a> {
    open: bool,
    device: &'a wgpu::Device,
}

impl<'a> WgpuErrorScope<'a> {
    pub fn start(device: &'a wgpu::Device) -> Self {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        // TODO(gfx-rs/wgpu#4866): Internal is missing!
        Self { device, open: true }
    }

    pub fn end(
        mut self,
    ) -> [impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static; 2] {
        self.open = false;
        [self.device.pop_error_scope(), self.device.pop_error_scope()]
    }
}

impl<'a> Drop for WgpuErrorScope<'a> {
    fn drop(&mut self) {
        if self.open {
            drop(self.device.pop_error_scope());
            drop(self.device.pop_error_scope());
        }
    }
}
