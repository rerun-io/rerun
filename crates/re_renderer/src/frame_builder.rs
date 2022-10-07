use crate::context::{RenderContext, RenderPipelineHandle};

/// Mirrors the GPU contents of a frame-global uniform buffer.
/// Contains information that is constant for a single frame like camera.
/// (does not contain information that is special to a particular renderer or global to the Context)
//struct FrameUniformBuffer {
// TODO(andreas): camera matrix and the like.
// What does not go here are even more global things like
//}

/// The highest level rendering block in `re_renderer`.
///
/// They are used to build up/collect various resources and then send them off for rendering.
/// Collecting objects in this fashion allows for re-use of common resources (e.g. camera)
#[derive(Default)]
pub struct FrameBuilder {
    render_pipeline: Option<RenderPipelineHandle>,
}

impl FrameBuilder {
    pub fn new() -> Self {
        FrameBuilder {
            render_pipeline: None,
        }
    }

    pub fn test_triangle(&mut self, ctx: &mut RenderContext, device: &wgpu::Device) -> &mut Self {
        self.render_pipeline = Some(ctx.request_render_pipeline(device));
        self
    }

    /// Consumes the frame builder and draws it as configured.
    ///
    /// TODO(andreas) getting the final eframe pass in here as the only rendering avenue is fine for simple stuff, but breaks
    /// once we need multi-pass rendering (some of them compute)
    /// Loosely related to
    /// * [egui #2022](https://github.com/emilk/egui/issues/2022)
    /// * [egui #2084](https://github.com/emilk/egui/issues/2084)
    pub fn draw<'a>(&self, ctx: &'a RenderContext, pass: &mut wgpu::RenderPass<'a>) {
        if let Some(handle) = self.render_pipeline {
            let render_pipeline = ctx.render_pipeline(handle);

            if let Some(render_pipeline) = render_pipeline {
                pass.set_pipeline(render_pipeline);
                pass.draw(0..3, 0..1);
            }
        }
    }
}
