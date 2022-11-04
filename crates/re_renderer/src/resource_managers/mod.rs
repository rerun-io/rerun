//! Resource managers are concerned with mapping (typically) higher level user data to
//! their Gpu representation.
//!
//! They facilitate lazy gpu upload and resource usage.
//!
//!
//! This is in contrast to the `crate::resource_pools` which are exclusively concerned with
//! low level gpu resources and their efficient allocation.

pub mod mesh_manager;
pub mod texture_manager;

mod resource_manager;
pub use resource_manager::{ResourceHandle, ResourceLifeTime, ResourceManagerError};
