//! Resource managers are concerned with mapping (typically) higher level user data to
//! their Gpu representation.
//!
//! They facilitate lazy gpu upload and resource usage.
//!
//!
//! This is in contrast to the [`crate::resource_pools`] which are exclusively concerned with
//! low level gpu resources and their efficient allocation.

pub mod mesh_manager;
//pub mod texture_manager; // TODO: WIP

/// Handle to a resource that is stored in a
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ResourceHandle<InnerHandle: slotmap::Key> {
    /// Handle that is valid until user explicitly removes the resource from respective resource manager.
    LongLived(InnerHandle),

    /// Handle that is valid for a single frame
    Frame {
        key: InnerHandle,
        /// This handle is only valid for this frame.
        /// Querying it during any other frame will fail.
        valid_frame_index: u64,
    },
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ResourceManagerError {
    #[error("The requested resource is no longer valid. It was valid for the frame index {current_frame_index}, but the current frame index is {valid_frame_index}")]
    ExpiredResource {
        current_frame_index: u64,
        valid_frame_index: u64,
    },

    #[error("The requested resource isn't available because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}
