//! Extensions for the [`RenderContext`] for testing.
//!
use crate::{RenderConfig, RenderContext, device_caps};

impl RenderContext {
    /// Creates a new [`RenderContext`] for testing.
    pub fn new_test() -> Self {
        let instance = wgpu::Instance::new(&device_caps::testing_instance_descriptor());
        let adapter = device_caps::select_testing_adapter(&instance);
        let device_caps = device_caps::DeviceCaps::from_adapter(&adapter)
            .expect("Failed to determine device capabilities");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&device_caps.device_descriptor()))
                .expect("Failed to request device.");

        Self::new(
            &adapter,
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            |_| RenderConfig::testing(),
        )
        .expect("Failed to create RenderContext")
    }

    /// Executes a test frame.
    ///
    /// Note that this "executes" a frame in thus far that it doesn't necessarily draw anything,
    /// depending on what the callback does.
    pub fn execute_test_frame<I>(&mut self, create_gpu_work: impl FnOnce(&mut Self) -> I)
    where
        I: IntoIterator<Item = wgpu::CommandBuffer>,
    {
        self.begin_frame();
        let command_buffers = create_gpu_work(self);
        self.before_submit();
        self.queue.submit(command_buffers);

        // Wait for all GPU work to finish.
        self.device
            .poll(wgpu::PollType::Wait)
            .expect("Failed to wait for GPU work to finish");

        // Start a new frame in order to handle the previous' frame errors.
        self.begin_frame();
    }
}
