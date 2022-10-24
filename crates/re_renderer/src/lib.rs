//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.

pub mod context;
pub mod renderer;
pub mod view_builder;

mod debug_label;
mod global_bindings;
mod resource_pools;
mod wgsl_types;

mod file_system;
pub use self::file_system::{get_filesystem, FileSystem};
#[allow(unused_imports)] // they can be handy from time to time
pub(crate) use self::file_system::{MemFileSystem, OsFileSystem};

mod file_resolver;
pub use self::file_resolver::{FileResolver, ImportClause, SearchPath};

mod file_server;
pub use self::file_server::{FileContentsHandle, FileServer};

mod workspace_shaders;
