use std::sync::atomic::{AtomicU64, Ordering};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PoolError {
    #[error("Requested resource isn't available because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}

/// A resource that can be owned & lifetime tracked by a resource pool.
pub(crate) trait GpuResource {
    /// Called every time a resource handle was resolved to its [`Resource`] object.
    fn on_handle_resolve(&self, _current_frame_index: u64) {}
}

// TODO(andreas): Make all resources usage tracked
/// A resource that keeps track of the last frame it was used.
pub(crate) trait UsageTrackedResource {
    fn last_frame_used(&self) -> &AtomicU64;
}

impl<T: UsageTrackedResource> GpuResource for T {
    fn on_handle_resolve(&self, current_frame_index: u64) {
        self.last_frame_used()
            .fetch_max(current_frame_index, Ordering::Release);
    }
}
