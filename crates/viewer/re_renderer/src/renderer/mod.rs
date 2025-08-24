mod compositor;
mod debug_overlay;
mod depth_cloud;
mod generic_skybox;
mod lines;
mod mesh_renderer;
mod point_cloud;
mod rectangles;
mod test_triangle;
mod world_grid;

pub use self::depth_cloud::{DepthCloud, DepthCloudDrawData, DepthCloudRenderer, DepthClouds};
pub use debug_overlay::{DebugOverlayDrawData, DebugOverlayError, DebugOverlayRenderer};
pub use generic_skybox::{GenericSkyboxDrawData, GenericSkyboxType};
pub use lines::{LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags};
pub use mesh_renderer::{GpuMeshInstance, MeshDrawData};
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
};
pub use rectangles::{
    ColorMapper, ColormappedTexture, RectangleDrawData, RectangleOptions, ShaderDecoding,
    TextureFilterMag, TextureFilterMin, TexturedRect,
};
pub use test_triangle::TestTriangleDrawData;
pub use world_grid::{WorldGridConfiguration, WorldGridDrawData, WorldGridRenderer};

pub mod gpu_data {
    pub use super::lines::gpu_data::{LineStripInfo, LineVertex};
    pub use super::point_cloud::gpu_data::PositionRadius;
}

pub(crate) use compositor::CompositorDrawData;
pub(crate) use mesh_renderer::MeshRenderer;

// ------------

use crate::{
    DrawableCollector,
    context::RenderContext,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{GpuRenderPipelinePoolAccessor, PoolError},
};

pub type DrawDataDrawableKey = u32;

/// A single drawable item within a given [`DrawData`].
///
/// The general expectation is that there's a rough one to one relationship between
/// drawables and drawcalls within a single [`DrawPhase`].
#[derive(Debug, Clone, Copy)]
pub struct DrawDataDrawable {
    /// Used for sorting drawables within a [`DrawPhase`].
    ///
    /// Low values mean closer, high values mean further away from the camera.
    /// This is typically simply the squared scene space distance to the observer,
    /// but may also be a 2D layer index or similar.
    ///
    /// Sorting for NaN is considered undefined.
    pub distance_sort_key: f32,

    /// Key for identifying the drawable within a given draw data.
    ///
    /// The meaning of this is dependent on the draw phase type.
    pub intra_draw_data_key: DrawDataDrawableKey,
}

impl DrawDataDrawable {
    #[inline]
    pub fn from_affine(
        view_info: &DrawableCollectionViewInfo,
        world_from_rdf: &glam::Affine3A,
        intra_draw_data_key: DrawDataDrawableKey,
    ) -> Self {
        Self::from_world_position(view_info, world_from_rdf.translation, intra_draw_data_key)
    }

    #[inline]
    pub fn from_world_position(
        view_info: &DrawableCollectionViewInfo,
        world_position: glam::Vec3A,
        intra_draw_data_key: DrawDataDrawableKey,
    ) -> Self {
        Self {
            distance_sort_key: world_position.distance_squared(view_info.camera_world_position),
            intra_draw_data_key,
        }
    }
}

/// Information about the view for which can be taken into account when collecting drawables.
pub struct DrawableCollectionViewInfo {
    /// The position of the camera in world space.
    pub camera_world_position: glam::Vec3A,
}

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
///
/// Each [`DrawData`] produces one or more [`DrawDataDrawable`]s for each view & phase.
///
/// Valid only for the frame in which it was created (may use temp allocations!).
//
// TODO(andreas): As of writing we don't actually use temp allocations. We should either drop
//               the single-frame validity assumption or enforce it!
// TODO(andreas): Architecturally we're not far from re-using draw across several views.
//                Only `QueueableDrawData` consuming draw data right now is preventing this.
pub trait DrawData {
    type Renderer: Renderer<RendererDrawData = Self> + Send + Sync;

    /// Collects all drawables for all phases of a specific view.
    ///
    /// Draw data implementations targeting several draw phases at once may choose to batch differently for each of them.
    ///
    /// Note that depending on the draw phase, drawables may be sorted differently or not at all.
    // TODO(andreas): This might also be the right place to introduce frustum culling by extending the view info.
    // on the flip side, we already put quite a bit of work into building up the draw data, not all of which is view-independent today (but it should be).
    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    );
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum DrawError {
    #[error(transparent)]
    Pool(#[from] PoolError),
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different over multiple [`Renderer::draw`] invocations is stored in [`DrawData`].
pub trait Renderer {
    type RendererDrawData: DrawData;

    fn create_renderer(ctx: &RenderContext) -> Self;

    // TODO(andreas): Some Renderers need to create their own passes, need something like this for that.

    /// Called once per phase given by [`Renderer::participated_phases`].
    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_data: &Self::RendererDrawData,
    ) -> Result<(), DrawError>;

    /// Combination of flags indicating in which phases [`Renderer::draw`] should be called.
    // TODO: this is obsolete with draw data collection
    fn participated_phases() -> &'static [DrawPhase];
}

/// Gets or creates a vertex shader module for drawing a screen filling triangle.
///
/// The entry point of this shader is `main`.
pub fn screen_triangle_vertex_shader(
    ctx: &RenderContext,
) -> crate::wgpu_resources::GpuShaderModuleHandle {
    ctx.gpu_resources.shader_modules.get_or_create(
        ctx,
        &include_shader_module!("../../shader/screen_triangle.wgsl"),
    )
}
