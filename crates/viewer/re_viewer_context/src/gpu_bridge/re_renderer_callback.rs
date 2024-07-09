slotmap::new_key_type! { pub struct ViewBuilderHandle; }

pub fn new_renderer_callback(
    view_builder: re_renderer::ViewBuilder,
    viewport: egui::Rect,
    clear_color: re_renderer::Rgba,
) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(
        viewport,
        ReRendererCallback {
            view_builder,
            clear_color,
        },
    )
}

struct ReRendererCallback {
    view_builder: re_renderer::ViewBuilder,
    clear_color: re_renderer::Rgba,
}

impl egui_wgpu::CallbackTrait for ReRendererCallback {
    // TODO(andreas): Prepare callbacks should run in parallel.
    //                Command buffer recording may be fairly expensive in the future!
    //                Sticking to egui's current model, each prepare callback could fork of a task and in finish_prepare we wait for them.
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        paint_callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(ctx) = paint_callback_resources.get::<re_renderer::RenderContext>() else {
            re_log::error_once!(
                "Failed to execute egui prepare callback. No render context available."
            );
            return Vec::new();
        };

        match self.view_builder.draw(ctx, self.clear_color) {
            Ok(command_buffer) => vec![command_buffer],
            Err(err) => {
                re_log::error_once!("Failed to fill view builder: {err}");
                // TODO(andreas): It would be nice to paint an error message instead.
                Vec::new()
            }
        }
    }

    fn finish_prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(ctx) = callback_resources.get_mut::<re_renderer::RenderContext>() else {
            re_log::error_once!(
                "Failed to execute egui prepare callback. No render context available."
            );
            return Vec::new();
        };

        // We don't own the render pass that renders the egui ui.
        // But we *still* need to somehow ensure that all resources used in callbacks drawing to it,
        // are longer lived than the pass itself.
        // This is a bit of a conundrum since we can't store a lock guard in the callback resources.
        // So instead, we work around this by moving the render pipelines out of their lock!
        // TODO(gfx-rs/wgpu#1453): Future wgpu versions will lift this restriction and will allow us to remove this workaround.
        if ctx.active_frame.pinned_render_pipelines.is_none() {
            let render_pipelines = ctx.gpu_resources.render_pipelines.take_resources();
            ctx.active_frame.pinned_render_pipelines = Some(render_pipelines);
        }

        Vec::new()
    }

    fn paint<'a>(
        &'a self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        paint_callback_resources: &'a egui_wgpu::CallbackResources,
    ) {
        let Some(ctx) = paint_callback_resources.get::<re_renderer::RenderContext>() else {
            // TODO(#4433): Shouldn't show up like this.
            re_log::error_once!(
                "Failed to execute egui draw callback. No render context available."
            );
            return;
        };
        let Some(render_pipelines) = ctx.active_frame.pinned_render_pipelines.as_ref() else {
            // TODO(#4433): Shouldn't show up like this.
            re_log::error_once!(
                "Failed to execute egui draw callback. Render pipelines weren't transferred out of the pool first."
            );
            return;
        };

        self.view_builder
            .composite(ctx, render_pipelines, render_pass);
    }
}
