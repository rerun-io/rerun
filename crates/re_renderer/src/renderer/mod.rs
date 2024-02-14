mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawData;

mod lines;
pub use lines::{
    gpu_data::LineVertex, LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags,
    LineStripInfo,
};

mod point_cloud;
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
    PositionRadius,
};

mod depth_cloud;
pub use self::depth_cloud::{DepthCloud, DepthCloudDrawData, DepthCloudRenderer, DepthClouds};

mod test_triangle;
pub use test_triangle::TestTriangleDrawData;

mod rectangles;
pub use rectangles::{
    ColorMapper, ColormappedTexture, RectangleDrawData, RectangleOptions, ShaderDecoding,
    TextureFilterMag, TextureFilterMin, TexturedRect,
};

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawData, MeshInstance};

mod compositor;
pub(crate) use compositor::CompositorDrawData;

mod debug_overlay;
pub use debug_overlay::{DebugOverlayDrawData, DebugOverlayError, DebugOverlayRenderer};

use crate::{
    context::RenderContext,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{self, GpuRenderPipelinePoolAccessor, PoolError},
    DebugLabel,
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
///
/// Valid only for the frame in which it was created (typically uses temp allocations!).
pub trait DrawData {
    type Renderer: Renderer<RendererDrawData = Self> + Send + Sync;
}

#[derive(thiserror::Error, Debug)]
pub enum DrawError {
    #[error(transparent)]
    Pool(#[from] PoolError),
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in [`DrawData`].
pub trait Renderer {
    type RendererDrawData: DrawData;

    fn create_renderer(ctx: &RenderContext) -> Self;

    // TODO(andreas): Some Renderers need to create their own passes, need something like this for that.

    /// Called once per phase given by [`Renderer::participated_phases`].
    fn draw<'a>(
        &self,
        render_pipelines: &'a GpuRenderPipelinePoolAccessor<'a>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> Result<(), DrawError>;

    /// Combination of flags indicating in which phases [`Renderer::draw`] should be called.
    fn participated_phases() -> &'static [DrawPhase];
}

/// Gets or creates a vertex shader module for drawing a screen filling triangle.
pub fn screen_triangle_vertex_shader(
    ctx: &RenderContext,
) -> crate::wgpu_resources::GpuShaderModuleHandle {
    ctx.gpu_resources.shader_modules.get_or_create(
        ctx,
        &include_shader_module!("../../shader/screen_triangle.wgsl"),
    )
}

/// Texture size for storing a given amount of data.
///
/// For WebGL compatibility we sometimes have to use textures instead of buffers.
/// We call these textures "data textures".
/// This method determines the size of a data texture holding a given number of used texels.
/// Each texel is typically a single data entry (think `struct`).
///
/// `max_texture_dimension_2d` must be a power of two and is the maximum supported size of 2D textures.
///
/// For convenience, the returned texture size has a width such that its
/// row size in bytes is a multiple of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`.
/// This makes it a lot easier to copy data from a continuous buffer to the texture.
/// If we wouldn't do that, we'd need to do a copy for each row in some cases.
pub fn data_texture_size(
    format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> wgpu::Extent3d {
    debug_assert!(max_texture_dimension_2d.is_power_of_two());
    debug_assert!(!format.has_depth_aspect());
    debug_assert!(!format.has_stencil_aspect());
    debug_assert!(!format.is_compressed());

    let texel_size_in_bytes = format
        .block_copy_size(None)
        .expect("Depth/stencil formats are not supported as data textures");

    // Our data textures are usually accessed in a linear fashion, so ideally we'd be using a 1D texture.
    // However, 1D textures are very limited in size on many platforms, we we have to use 2D textures instead.
    // 2D textures perform a lot better when their dimensions are powers of two, so we'll strictly stick to that even
    // when it seems to cause memory overhead.

    // We fill row by row. With the power-of-two requirement, this is the optimal strategy:
    // if there were a texture with less padding that uses half the width,
    // then we'd need to increase the height. We can't increase without doubling it, thus creating a texture
    // with the exact same mount of padding as before.

    let width = if num_texels_written < max_texture_dimension_2d {
        num_texels_written
            .next_power_of_two()
            // For too few number of written texels, or too small texels we might need to increase the size to stay
            // above a row **byte** size of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`.
            // Note that this implies that for very large texels, we need less wide textures to stay above this limit.
            // (width is in number of texels, but alignment cares about bytes!)
            .next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT / texel_size_in_bytes)
    } else {
        max_texture_dimension_2d
    };

    let height = num_texels_written.div_ceil(width);

    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// Texture descriptor for data storage.
///
/// See [`data_texture_size`]
pub fn data_texture_desc(
    label: impl Into<DebugLabel>,
    format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> wgpu_resources::TextureDesc {
    wgpu_resources::TextureDesc {
        label: label.into(),
        size: data_texture_size(format, num_texels_written, max_texture_dimension_2d),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    }
}

/// Pendent to [`data_texture_size`] for determining the element size (==texels on data texture)
/// need to be in a buffer that fills an entire data texture.
pub fn data_texture_source_buffer_element_count(
    texture_format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> usize {
    let data_texture_size =
        data_texture_size(texture_format, num_texels_written, max_texture_dimension_2d);
    let element_count = data_texture_size.width as usize * data_texture_size.height as usize;

    debug_assert!(element_count >= num_texels_written as usize);

    element_count
}
