//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

pub mod config;
pub mod importer;
pub mod mesh;
pub mod renderer;
pub mod resource_managers;
pub mod texture_info;
pub mod video;
pub mod view_builder;

mod allocator;
mod color;
mod colormap;
mod context;
mod debug_label;
mod depth_offset;
mod draw_phases;
mod error_handling;
mod file_resolver;
mod file_server;
mod file_system;
mod global_bindings;
mod line_drawable_builder;
mod point_cloud_builder;
mod queueable_draw_data;
mod rect;
mod size;
mod transform;
mod wgpu_buffer_types;
mod wgpu_resources;

#[cfg(not(load_shaders_from_disk))]
#[rustfmt::skip] // it's auto-generated
mod workspace_shaders;

// ---------------------------------------------------------------------------
// Exports

use allocator::GpuReadbackBuffer;

pub use allocator::{CpuWriteGpuReadError, GpuReadbackIdentifier};
pub use color::Rgba32Unmul;
pub use colormap::{
    colormap_cyan_to_yellow_srgb, colormap_inferno_srgb, colormap_magma_srgb, colormap_plasma_srgb,
    colormap_srgb, colormap_turbo_srgb, colormap_viridis_srgb, grayscale_srgb, Colormap,
};
pub use context::{adapter_info_summary, RenderContext, RenderContextError};
pub use debug_label::DebugLabel;
pub use depth_offset::DepthOffset;
pub use importer::{CpuMeshInstance, CpuModel, CpuModelMeshKey};
pub use line_drawable_builder::{LineDrawableBuilder, LineStripBuilder};
pub use point_cloud_builder::{PointCloudBatchBuilder, PointCloudBuilder};
pub use queueable_draw_data::QueueableDrawData;
pub use rect::{RectF32, RectInt};
pub use size::Size;
pub use texture_info::Texture2DBufferInfo;
pub use transform::RectTransform;
pub use view_builder::ViewBuilder;
pub use wgpu_resources::WgpuResourcePoolStatistics;

use draw_phases::DrawPhase;
pub use draw_phases::{
    OutlineConfig, OutlineMaskPreference, PickingLayerId, PickingLayerInstanceId,
    PickingLayerObjectId, PickingLayerProcessor, ScreenshotProcessor,
};

pub use self::file_system::{get_filesystem, FileSystem};
#[allow(unused_imports)] // they can be handy from time to time
use self::file_system::{MemFileSystem, OsFileSystem};

pub use self::file_resolver::{
    new_recommended as new_recommended_file_resolver, FileResolver, ImportClause,
    RecommendedFileResolver, SearchPath,
};
pub use self::file_server::FileServer;

// Re-export used color types directly.
pub use ecolor::{Color32, Hsva, Rgba};

pub mod external {
    pub use re_video;
    pub use wgpu;
}

// ---------------------------------------------------------------------------

// Make Arrow integration as transparent as possible.

#[cfg(feature = "arrow")]
pub type Buffer<T> = arrow2::buffer::Buffer<T>;

#[cfg(not(feature = "arrow"))]
pub type Buffer<T> = Vec<T>;

// ---------------------------------------------------------------------------

/// Pad `RGB` to `RGBA` with the given alpha.
pub fn pad_rgb_to_rgba<T: Copy>(rgb: &[T], alpha: T) -> Vec<T> {
    re_tracing::profile_function!();
    if cfg!(debug_assertions) {
        // fastest version in debug builds.
        // 5x faster in debug builds, but 2x slower in release
        let mut rgba = vec![alpha; rgb.len() / 3 * 4];
        for i in 0..(rgb.len() / 3) {
            rgba[4 * i] = rgb[3 * i];
            rgba[4 * i + 1] = rgb[3 * i + 1];
            rgba[4 * i + 2] = rgb[3 * i + 2];
        }
        rgba
    } else {
        // fastest version in optimized builds
        rgb.chunks_exact(3)
            .flat_map(|chunk| [chunk[0], chunk[1], chunk[2], alpha])
            .collect()
    }
}
