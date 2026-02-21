//! Rerun's renderer.
//!
//! A wgpu based renderer [wgpu](https://github.com/gfx-rs/wgpu/) for all your visualization needs.
//! Used in `re_runner` to display the contents of any view contents other than pure UI.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!
//! ## Draw recording overview
//!
//! [`ViewBuilder`] are the main entry point for all draw operations.
//! Each [`ViewBuilder`] represents a rectangular screen area that is composited into the target surface
//! via [`ViewBuilder::composite`].
//!
//! A user supplies [`renderer::DrawData`]s to the [`ViewBuilder`].
//! Upon submission, the [`ViewBuilder`] collects the [`Drawable`]s from the [`QueueableDrawData`] and
//! adds them to the appropriate work queues of each draw phase.
//! [`Drawable`]s map roughly 1:1 to wgpu draw calls and have a [`renderer::DrawData`] type specific payload
//! that identifies them within their [`renderer::DrawData`].
//!
//! Depending on the [`DrawPhase`] sorting of drawables may occur:
//! for instance [`DrawPhase::Transparent`] sorts far to near to facilitate blending, whereas other phases aggressively
//! bundle by [`renderer::DrawData`] types to minimize state changes.
//!
//! Each [`renderer::DrawData`] is associated with a single [`renderer::Renderer`].
//! These encapsulate the knowledge (i.e. renderpipelines etc.) of how to render a certain kind of primitive.
//! Unlike [`renderer::DrawData`]s, [`renderer::Renderer`]s are immutable and long-lived.

// TODO(#3408): remove unwrap()
#![expect(clippy::unwrap_used)]

mod allocator;
pub mod device_caps;
pub mod importer;
pub mod mesh;
pub mod renderer;
pub mod resource_managers;
pub mod texture_info;
pub mod video;
pub mod view_builder;
pub mod wgpu_buffer_types;

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
mod wgpu_resources;

#[cfg(test)]
mod context_test;

#[cfg(not(load_shaders_from_disk))]
#[rustfmt::skip] // it's auto-generated
mod workspace_shaders;

// ---------------------------------------------------------------------------
// Exports

use allocator::GpuReadbackBuffer;
pub use allocator::{
    CpuWriteGpuReadError, DataTextureSource, DataTextureSourceWriteError, GpuReadbackIdentifier,
    create_and_fill_uniform_buffer, create_and_fill_uniform_buffer_batch,
};
pub use color::{Rgba32Unmul, UnalignedColor32};
pub use colormap::{
    Colormap, colormap_cyan_to_yellow_srgb, colormap_inferno_srgb, colormap_magma_srgb,
    colormap_plasma_srgb, colormap_srgb, colormap_turbo_srgb, colormap_viridis_srgb,
    grayscale_srgb,
};
pub use context::{
    MsaaMode, RenderConfig, RenderContext, RenderContextError, RendererTypeId, adapter_info_summary,
};
pub use debug_label::DebugLabel;
pub use depth_offset::DepthOffset;
pub use draw_phases::{
    DrawPhase, DrawPhaseManager, Drawable, DrawableCollector, OutlineConfig, OutlineMaskPreference,
    OutlineMaskProcessor, PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId,
    PickingLayerProcessor, ScreenshotProcessor,
};
pub use resource_managers::AlphaChannelUsage;
// Re-export used color types directly.
pub use ecolor::{Color32, Hsva, Rgba};
pub use global_bindings::GlobalBindings;
pub use importer::{CpuMeshInstance, CpuModel, CpuModelMeshKey};
pub use line_drawable_builder::{LineBatchBuilder, LineDrawableBuilder, LineStripBuilder};
pub use point_cloud_builder::{PointCloudBatchBuilder, PointCloudBuilder};
pub use queueable_draw_data::QueueableDrawData;
pub use rect::{RectF32, RectInt};
pub use size::Size;
pub use texture_info::Texture2DBufferInfo;
pub use transform::RectTransform;
pub use view_builder::{RenderMode, ViewBuilder, ViewPickingConfiguration};
pub use wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuPipelineLayoutPool, GpuRenderPipelineHandle, GpuRenderPipelinePool,
    GpuRenderPipelinePoolAccessor, GpuShaderModuleHandle, GpuShaderModulePool, GpuTexture,
    GpuTextureHandle, PipelineLayoutDesc, RenderPipelineDesc, ShaderModuleDesc, VertexBufferLayout,
    WgpuResourcePoolStatistics,
};

pub use self::file_resolver::{
    FileResolver, ImportClause, RecommendedFileResolver, SearchPath,
    new_recommended as new_recommended_file_resolver,
};
pub use self::file_server::FileServer;
#[allow(clippy::allow_attributes, unused_imports)] // they can be handy from time to time
use self::file_system::MemFileSystem;
pub use self::file_system::{FileSystem, get_filesystem};

#[cfg(load_shaders_from_disk)]
use self::file_system::OsFileSystem;

pub mod external {
    pub use {anyhow, bytemuck, re_video, smallvec, wgpu};
}

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
