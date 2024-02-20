//! High level GPU memory allocators.
//!
//! In contrast to the buffer pools in [`crate::wgpu_resources`], every allocator in here
//! follows some more complex strategy for efficient re-use and sub-allocation of wgpu resources.

mod cpu_write_gpu_read_belt;
mod data_texture_source;
mod gpu_readback_belt;
mod uniform_buffer_fill;

pub use cpu_write_gpu_read_belt::{
    CpuWriteGpuReadBelt, CpuWriteGpuReadBuffer, CpuWriteGpuReadError,
};
pub use data_texture_source::{data_texture_desc, DataTextureSource};
pub use gpu_readback_belt::{
    GpuReadbackBelt, GpuReadbackBuffer, GpuReadbackError, GpuReadbackIdentifier,
};
pub use uniform_buffer_fill::{
    create_and_fill_uniform_buffer, create_and_fill_uniform_buffer_batch,
};
