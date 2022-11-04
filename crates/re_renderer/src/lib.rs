//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.

pub mod config;
pub mod importer;
pub mod renderer;
pub mod view_builder;

mod context;
pub use context::RenderContext;

mod debug_label;
pub use self::debug_label::DebugLabel;

mod global_bindings;

pub mod mesh;
pub mod mesh_manager;

mod resource_pools;
mod wgpu_buffer_types;

mod file_system;
pub use self::file_system::{get_filesystem, FileSystem};
#[allow(unused_imports)] // they can be handy from time to time
pub(crate) use self::file_system::{MemFileSystem, OsFileSystem};

mod file_resolver;
pub use self::file_resolver::{
    new_recommended as new_recommended_file_resolver, FileResolver, ImportClause,
    RecommendedFileResolver, SearchPath,
};

mod file_server;
pub use self::file_server::FileServer;

#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // wasm or release builds
mod workspace_shaders;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
mod error_tracker;
