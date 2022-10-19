//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.

pub mod context;
pub mod frame_builder;

mod debug_label;
mod global_bindings;
mod renderer;
mod resource_pools;
mod wgsl_types;
