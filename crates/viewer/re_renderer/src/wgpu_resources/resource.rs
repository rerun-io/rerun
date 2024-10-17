use std::sync::atomic::AtomicU64;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    #[error("Requested resource isn't available because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,

    #[error("The passed descriptor doesn't refer to a known resource")]
    UnknownDescriptor,
}

pub struct ResourceStatistics {
    /// Frame index in which this resource was (re)created.
    pub frame_created: u64,

    /// Frame index in which a handle to this resource was last resolved.
    ///
    /// Note that implicit usage via other resources is *not* tracked.
    pub last_frame_used: AtomicU64,
}
