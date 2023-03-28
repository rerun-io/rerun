//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

pub mod config;
pub mod importer;
pub mod mesh;
pub mod renderer;
pub mod resource_managers;
pub mod view_builder;

mod allocator;
mod color;
mod colormap;
mod context;
mod debug_label;
mod depth_offset;
mod global_bindings;
mod line_strip_builder;
mod point_cloud_builder;
mod size;
mod wgpu_buffer_types;
mod wgpu_resources;

pub use allocator::{GpuReadbackBelt, GpuReadbackBuffer, GpuReadbackBufferIdentifier};
pub use color::Rgba32Unmul;
pub use colormap::{
    colormap_inferno_srgb, colormap_magma_srgb, colormap_plasma_srgb, colormap_srgb,
    colormap_turbo_srgb, colormap_viridis_srgb, grayscale_srgb, ColorMap,
};
pub use context::RenderContext;
pub use debug_label::DebugLabel;
pub use depth_offset::DepthOffset;
pub use line_strip_builder::{LineStripBuilder, LineStripSeriesBuilder};
pub use point_cloud_builder::{PointCloudBatchBuilder, PointCloudBuilder};
pub use size::Size;
pub use view_builder::{AutoSizeConfig, ScheduledScreenshot};
pub use wgpu_resources::WgpuResourcePoolStatistics;

mod draw_phases;
pub(crate) use draw_phases::DrawPhase;
pub use draw_phases::{
    OutlineConfig, OutlineMaskPreference, PickingLayerInstanceId, PickingLayerObjectId,
    ScheduledPickingRect,
};

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
#[rustfmt::skip] // it's auto-generated
mod workspace_shaders;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
mod error_tracker;

// Re-export used color types.
pub use ecolor::{Color32, Rgba};

// ---------------------------------------------------------------------------

// Make Arrow integration as transparent as possible.

#[cfg(feature = "arrow")]
pub type Buffer<T> = arrow2::buffer::Buffer<T>;

#[cfg(not(feature = "arrow"))]
pub type Buffer<T> = Vec<T>;

// ---------------------------------------------------------------------------

/// Profiling macro for puffin
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for puffin
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
