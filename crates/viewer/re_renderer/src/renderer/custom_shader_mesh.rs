//! Custom shader mesh renderer.
//!
//! Renders mesh geometry with a user-provided WGSL fragment shader.
//! Reuses the standard mesh vertex buffers and instance data layout
//! from the regular mesh renderer, but replaces the fragment shader.
//!
//! This is an experimental feature.

use std::sync::Arc;

use crate::context::RenderContext;
use crate::draw_phases::DrawPhase;
use crate::mesh::GpuMesh;
use crate::wgpu_resources::GpuShaderModuleHandle;
use crate::{Color32, OutlineMaskPreference, PickingLayerId};

/// A single instance of a mesh rendered with a custom shader.
#[derive(Clone)]
pub struct CustomShaderMeshInstance {
    /// The GPU mesh to render.
    pub gpu_mesh: Arc<GpuMesh>,

    /// Transform from mesh space to world space.
    pub world_from_mesh: glam::Affine3A,

    /// Outline mask for selection highlighting.
    pub outline_mask_ids: OutlineMaskPreference,

    /// Picking layer ID for mouse interaction.
    pub picking_layer_id: PickingLayerId,

    /// Additive tint color.
    pub additive_tint: Color32,

    /// The user's custom WGSL fragment shader handle.
    pub shader_module: GpuShaderModuleHandle,

    /// Hash of the shader source for pipeline caching.
    pub shader_hash: u64,

    /// Optional custom bind group for shader parameters.
    /// Bound at group 2 (group 0 = global, group 1 = mesh material).
    pub custom_bind_group: Option<Arc<wgpu::BindGroup>>,

    /// Optional custom bind group layout (must match `custom_bind_group`).
    pub custom_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
}

/// Create an inline shader module from WGSL source, cached by content hash.
pub fn create_custom_shader_module(
    ctx: &RenderContext,
    label: &str,
    wgsl_source: &str,
) -> (GpuShaderModuleHandle, u64) {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = ahash::AHasher::default();
    wgsl_source.hash(&mut hasher);
    let content_hash = hasher.finish();

    let handle = ctx.gpu_resources.shader_modules.get_or_create_inline(
        &ctx.device,
        label,
        wgsl_source,
        content_hash,
    );

    (handle, content_hash)
}

/// Returns the draw phases that a custom shader mesh participates in.
pub fn custom_shader_draw_phases() -> enumset::EnumSet<DrawPhase> {
    DrawPhase::Opaque | DrawPhase::PickingLayer | DrawPhase::OutlineMask
}
