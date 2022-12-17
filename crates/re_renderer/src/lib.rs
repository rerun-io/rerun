//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.

pub mod config;
pub mod importer;
pub mod mesh;
pub mod renderer;
pub mod resource_managers;
pub mod view_builder;

mod context;
mod debug_label;
mod global_bindings;
mod line_strip_builder;
mod point_cloud_builder;
mod size;
mod wgpu_buffer_types;
mod wgpu_resources;

pub use context::RenderContext;
pub use debug_label::DebugLabel;
pub use line_strip_builder::{LineStripBuilder, LineStripSeriesBuilder};
pub use point_cloud_builder::{PointCloudBatchBuilder, PointCloudBuilder};
pub use size::Size;
pub use wgpu_resources::WgpuResourcePoolStatistics;

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

// Re-export used color types.
pub use ecolor::{Color32, Rgba};

// ---------------------------------------------------------------------------

// part of std, but unstable https://github.com/rust-lang/rust/issues/88581
pub(crate) const fn next_multiple_of(cur: u32, rhs: u32) -> u32 {
    match cur % rhs {
        0 => cur,
        r => cur + (rhs - r),
    }
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        puffin::profile_scope!($($arg)*);
    };
}
