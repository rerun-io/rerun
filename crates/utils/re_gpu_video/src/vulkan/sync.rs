//! Timeline semaphore ordering decode and copy submissions, and the host wait on them.

use std::sync::Arc;

use ash::vk;

use super::device::Device;

/// A timeline semaphore tracking the values it signaled.
pub struct TimelineSemaphore {
    device: Arc<Device>,
    raw: vk::Semaphore,

    /// The value most recently scheduled for signaling.
    value: u64,
}

impl TimelineSemaphore {
    #[expect(unsafe_code)]
    pub fn new(device: Arc<Device>) -> Result<Self, vk::Result> {
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let create_info = vk::SemaphoreCreateInfo::default().push_next(&mut type_info);

        // SAFETY: Plain creation, destroyed in drop.
        let raw = unsafe { device.raw.create_semaphore(&create_info, None) }?;
        Ok(Self {
            device,
            raw,
            value: 0,
        })
    }

    /// Submits one command buffer, signaling the next timeline value on completion
    /// and optionally waiting for an earlier value first. Returns the signal value.
    #[expect(unsafe_code)]
    pub fn submit(
        &mut self,
        queue: vk::Queue,
        command_buffer: vk::CommandBuffer,
        wait_value: Option<u64>,
    ) -> Result<u64, vk::Result> {
        let signal_value = self.value + 1;

        let wait_infos: Vec<vk::SemaphoreSubmitInfo<'_>> = wait_value
            .map(|value| {
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(self.raw)
                    .value(value)
                    .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            })
            .into_iter()
            .collect();
        let signal_infos = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.raw)
            .value(signal_value)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];
        let command_buffer_infos =
            [vk::CommandBufferSubmitInfo::default().command_buffer(command_buffer)];
        let submit = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&wait_infos)
            .command_buffer_infos(&command_buffer_infos)
            .signal_semaphore_infos(&signal_infos);

        // SAFETY: The command buffer is fully recorded, the queue is externally
        // synchronized by the caller's lock.
        unsafe {
            self.device
                .raw
                .queue_submit2(queue, &[submit], vk::Fence::null())?;
        }
        self.value = signal_value;
        Ok(signal_value)
    }

    /// Blocks until the semaphore reaches `value`.
    #[expect(unsafe_code)]
    pub fn wait(&self, value: u64) -> Result<(), vk::Result> {
        re_tracing::profile_function!();

        let semaphores = [self.raw];
        let values = [value];
        let wait_info = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);

        // SAFETY: Plain host wait.
        unsafe { self.device.raw.wait_semaphores(&wait_info, u64::MAX) }
    }

    /// Blocks until everything submitted so far completed. Used before teardown.
    pub fn wait_idle(&self) -> Result<(), vk::Result> {
        self.wait(self.value)
    }
}

impl Drop for TimelineSemaphore {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            self.device.raw.destroy_semaphore(self.raw, None);
        }
    }
}
