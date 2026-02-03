use re_mutex::Mutex;

slotmap::new_key_type! { pub struct ViewBuilderHandle; }

pub fn new_renderer_callback(
    view_builder: re_renderer::ViewBuilder,
    viewport: egui::Rect,
    clear_color: re_renderer::Rgba,
) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(
        viewport,
        ReRendererCallback {
            view_builder: Mutex::new(view_builder),
            clear_color,
        },
    )
}

struct ReRendererCallback {
    view_builder: Mutex<re_renderer::ViewBuilder>,
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

        match self.view_builder.lock().draw(ctx, self.clear_color) {
            Ok(command_buffer) => vec![command_buffer],
            Err(err) => {
                re_log::error_once!("Failed to fill view builder: {err}");
                // TODO(andreas): It would be nice to paint an error message instead.
                Vec::new()
            }
        }
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        paint_callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(ctx) = paint_callback_resources.get::<re_renderer::RenderContext>() else {
            // TODO(#4433): Shouldn't show up like this.
            re_log::error_once!(
                "Failed to execute egui draw callback. No render context available."
            );
            return;
        };
        self.view_builder.lock().composite(ctx, render_pass);
    }
}
