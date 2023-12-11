/// Wgpu device validation scope that auto closes when exiting the scope unless it was already closed.
///
/// The expectation is that the scope is manually closed, but this construct is useful to not accidentally
/// leave the scope open when returning early from a function.
pub struct SafeWgpuValidationScope<'a> {
    open: bool,
    device: &'a wgpu::Device,
}

impl<'a> SafeWgpuValidationScope<'a> {
    pub fn start(device: &'a wgpu::Device) -> Self {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        Self { device, open: true }
    }

    pub fn end(
        mut self,
    ) -> impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static {
        self.open = false;
        self.device.pop_error_scope()
    }
}

impl<'a> Drop for SafeWgpuValidationScope<'a> {
    fn drop(&mut self) {
        if self.open {
            drop(self.device.pop_error_scope());
        }
    }
}
