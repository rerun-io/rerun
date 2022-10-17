use anyhow::Context;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    context::*,
    renderer::{generic_skybox::*, test_triangle::*, tonemapper::*, Renderer},
    resource_pools::{bind_group_pool::BindGroupHandle, texture_pool::*},
};

/// The highest level rendering block in `re_renderer`.
///
/// They are used to build up/collect various resources and then send them off for rendering.
/// Collecting objects in this fashion allows for re-use of common resources (e.g. camera)
#[derive(Default)]
pub struct FrameBuilder {
    tonemapping_draw_data: TonemapperDrawData,
    test_triangle_draw_data: Option<TestTriangleDrawData>,
    generic_skybox_draw_data: Option<GenericSkyboxDrawData>,

    bind_group_0: BindGroupHandle,

    hdr_render_target: TextureHandle,
    depth_buffer: TextureHandle,
}

pub type SharedFrameBuilder = Arc<RwLock<FrameBuilder>>;

impl FrameBuilder {
    pub const FORMAT_HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const FORMAT_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

    pub fn new() -> Self {
        FrameBuilder {
            tonemapping_draw_data: Default::default(),
            test_triangle_draw_data: None,
            generic_skybox_draw_data: None,

            bind_group_0: BindGroupHandle::default(),

            hdr_render_target: TextureHandle::default(),
            depth_buffer: TextureHandle::default(),
        }
    }

    pub fn new_shared() -> SharedFrameBuilder {
        Arc::new(RwLock::new(FrameBuilder::new()))
    }

    pub fn setup_target(
        &mut self,
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> &mut Self {
        // TODO(andreas): Should tonemapping preferences go here as well? Likely!
        // TODO(andreas): How should we treat multisampling. Once we start it we also need to deal with MSAA resolves
        self.hdr_render_target = ctx.resource_pools.textures.request(
            device,
            &render_target_2d_desc(Self::FORMAT_HDR, width, height, 1),
        );
        self.depth_buffer = ctx.resource_pools.textures.request(
            device,
            &render_target_2d_desc(Self::FORMAT_DEPTH, width, height, 1),
        );

        self.tonemapping_draw_data = ctx
            .renderers
            .get_or_create::<Tonemapper>(&ctx.shared_renderer_data, &mut ctx.resource_pools, device)
            .prepare(
                &mut ctx.resource_pools,
                device,
                &TonemapperPrepareData {
                    hdr_target: self.hdr_render_target,
                },
            );

        self.bind_group_0 = ctx
            .shared_renderer_data
            .global_bindings
            .create_bind_group(&mut ctx.resource_pools, device);

        self
    }

    pub fn test_triangle(&mut self, ctx: &mut RenderContext, device: &wgpu::Device) -> &mut Self {
        let pools = &mut ctx.resource_pools;
        self.test_triangle_draw_data = Some(
            ctx.renderers
                .get_or_create::<TestTriangle>(&ctx.shared_renderer_data, pools, device)
                .prepare(pools, device, &TestTrianglePrepareData {}),
        );

        self
    }

    pub fn generic_skybox(&mut self, ctx: &mut RenderContext, device: &wgpu::Device) -> &mut Self {
        let pools = &mut ctx.resource_pools;
        self.generic_skybox_draw_data = Some(
            ctx.renderers
                .get_or_create::<GenericSkybox>(&ctx.shared_renderer_data, pools, device)
                .prepare(pools, device, &GenericSkyboxPrepareData {}),
        );

        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let color = ctx
            .resource_pools
            .textures
            .get(self.hdr_render_target)
            .context("hdr render target")?;
        let depth = ctx
            .resource_pools
            .textures
            .get(self.depth_buffer)
            .context("depth buffer")?;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame builder hdr pass"), // TODO(andreas): It would be nice to specify this from the outside so we know which view we're rendering
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: false, // discards the depth buffer after use, can be faster
                }),
                stencil_ops: None,
            }),
        });

        pass.set_bind_group(
            0,
            &ctx.resource_pools
                .bind_groups
                .get(self.bind_group_0)
                .context("get global bind group")?
                .bind_group,
            &[],
        );

        if let Some(test_triangle_data) = self.test_triangle_draw_data.as_ref() {
            let test_triangle_renderer = ctx
                .renderers
                .get::<TestTriangle>()
                .context("get test triangle renderer")?;
            test_triangle_renderer
                .draw(&ctx.resource_pools, &mut pass, test_triangle_data)
                .context("draw test triangle")?;
        }

        if let Some(generic_skybox_data) = self.generic_skybox_draw_data.as_ref() {
            let generic_skybox_renderer = ctx
                .renderers
                .get::<GenericSkybox>()
                .context("get skybox renderer")?;
            generic_skybox_renderer
                .draw(&ctx.resource_pools, &mut pass, generic_skybox_data)
                .context("draw skybox")?;
        }

        Ok(())
    }

    /// Applies tonemapping and draws the final result of a `FrameBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    pub fn finish<'a>(
        &self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> anyhow::Result<()> {
        pass.set_bind_group(
            0,
            &ctx.resource_pools
                .bind_groups
                .get(self.bind_group_0)
                .context("get global bind group")?
                .bind_group,
            &[],
        );

        let tonemapper = ctx
            .renderers
            .get::<Tonemapper>()
            .context("get tonemapper")?;
        tonemapper
            .draw(&ctx.resource_pools, pass, &self.tonemapping_draw_data)
            .context("perform tonemapping")
    }
}
