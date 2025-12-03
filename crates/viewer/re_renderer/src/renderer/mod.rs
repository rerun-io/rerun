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

pub use debug_overlay::{DebugOverlayDrawData, DebugOverlayError, DebugOverlayRenderer};
pub use generic_skybox::{GenericSkyboxDrawData, GenericSkyboxType};
pub use lines::{LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags};
pub use mesh_renderer::{GpuMeshInstance, MeshDrawData};
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
};
pub use rectangles::{
    ColorMapper, ColormappedTexture, RectangleDrawData, RectangleOptions, ShaderDecoding,
    TextureAlpha, TextureFilterMag, TextureFilterMin, TexturedRect,
};
pub use test_triangle::TestTriangleDrawData;
pub use world_grid::{WorldGridConfiguration, WorldGridDrawData, WorldGridRenderer};

pub use self::depth_cloud::{DepthCloud, DepthCloudDrawData, DepthCloudRenderer, DepthClouds};

pub mod gpu_data {
    pub use super::lines::gpu_data::{LineStripInfo, LineVertex};
    pub use super::point_cloud::gpu_data::PositionRadius;
}

pub(crate) use compositor::CompositorDrawData;
pub(crate) use mesh_renderer::MeshRenderer;

// ------------
use crate::{
    Drawable, DrawableCollector, QueueableDrawData,
    context::RenderContext,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{GpuRenderPipelinePoolAccessor, PoolError},
};

/// [`DrawData`] specific payload that is injected into the otherwise type agnostic [`crate::Drawable`].
pub type DrawDataDrawablePayload = u32;

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

    /// Key for identifying the drawable within the [`DrawData`] that produced it..
    ///
    /// This is effectively an arbitrary payload whose meaning is dependent on the drawable type
    /// but typically refers to instances or instance ranges within the draw data.
    pub draw_data_payload: DrawDataDrawablePayload,
}

impl DrawDataDrawable {
    #[inline]
    pub fn from_affine(
        view_info: &DrawableCollectionViewInfo,
        world_from_rdf: &glam::Affine3A,
        draw_data_payload: DrawDataDrawablePayload,
    ) -> Self {
        Self::from_world_position(view_info, world_from_rdf.translation, draw_data_payload)
    }

    #[inline]
    pub fn from_world_position(
        view_info: &DrawableCollectionViewInfo,
        world_position: glam::Vec3A,
        draw_data_payload: DrawDataDrawablePayload,
    ) -> Self {
        Self {
            distance_sort_key: world_position.distance_squared(view_info.camera_world_position),
            draw_data_payload,
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
//
// TODO(andreas): We're currently not re-using draw across several views & frames.
// Architecturally there's not much preventing this except for `QueueableDrawData` consuming `DrawData` right now.
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

/// A draw instruction specifies which drawables of a given [`DrawData`] should be rendered.
pub struct DrawInstruction<'a, D> {
    /// The draw data that produced the [`Self::drawables`].
    pub draw_data: &'a D,

    /// The drawables to render.
    ///
    /// It's guaranteed that all drawables originated from a single call to [`DrawData::collect_drawables`]
    /// of [`Self::draw_data`] but there are no guarantees on ordering.
    pub drawables: &'a [Drawable],
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different over multiple [`Renderer::draw`] invocations is stored in [`DrawData`].
pub trait Renderer {
    type RendererDrawData: DrawData + 'static;

    fn create_renderer(ctx: &RenderContext) -> Self;

    /// Called once per phase if there are any drawables for that phase.
    ///
    /// For each draw data reference, there's at most one [`DrawInstruction`].
    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError>;
}

/// Extension trait for [`Renderer`] that allows running draw instructions with type erased draw data.
pub(crate) trait RendererExt: Send + Sync {
    fn run_draw_instructions(
        &self,
        gpu_resources: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        type_erased_draw_instructions: &[DrawInstruction<'_, QueueableDrawData>],
    ) -> Result<(), DrawError>;

    /// Name of the renderer, used for debugging & error reporting.
    fn name(&self) -> &'static str;
}

impl<R: Renderer + Send + Sync> RendererExt for R {
    fn run_draw_instructions(
        &self,
        gpu_resources: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        type_erased_draw_instructions: &[DrawInstruction<'_, QueueableDrawData>],
    ) -> Result<(), DrawError> {
        let draw_instructions: Vec<DrawInstruction<'_, R::RendererDrawData>> =
            type_erased_draw_instructions
                .iter()
                .map(|type_erased_draw_instruction| DrawInstruction {
                    draw_data: type_erased_draw_instruction.draw_data.expect_downcast(),
                    drawables: type_erased_draw_instruction.drawables,
                })
                .collect();

        self.draw(gpu_resources, phase, pass, &draw_instructions)
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<R>()
    }
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
