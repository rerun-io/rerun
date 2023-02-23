//! High level GPU memory allocators.
//!
//! In contrast to the buffer pools in [`crate::wgpu_resources`], every allocator in here
//! follows some more complex strategy for efficient re-use and sub-allocation of wgpu resources.

mod cpu_write_gpu_read_belt;

pub use cpu_write_gpu_read_belt::{CpuWriteGpuReadBelt, CpuWriteGpuReadBuffer};
