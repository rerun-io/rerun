slotmap::new_key_type! { pub struct ViewBuilderHandle; }

type ViewBuilderMap = slotmap::SlotMap<ViewBuilderHandle, Option<re_renderer::ViewBuilder>>;

pub fn new_renderer_callback(
    render_ctx: &mut re_renderer::RenderContext,
    view_builder: re_renderer::ViewBuilder,
    viewport: egui::Rect,
    clear_color: re_renderer::Rgba,
) -> egui::PaintCallback {
    let composition_view_builder_map = render_ctx
        .active_frame
        .per_frame_data_helper
        .entry::<ViewBuilderMap>()
        .or_insert_with(Default::default);
    let view_builder_handle = composition_view_builder_map.insert(Some(view_builder));

    egui_wgpu::Callback::new_paint_callback(
        viewport,
        ReRendererCallback {
            view_builder: view_builder_handle,
            clear_color,
        },
    )
}

struct ReRendererCallback {
    // It would be nice to put the ViewBuilder in here directly, but this
    // struct is required to be Send/Sync and wgpu resources aren't on wasm.
    // Technically, we ignore this restriction by using the `fragile-send-sync-non-atomic-wasm` wgpu feature flag.
    //
    // However, in addition, we need to make sure that the ViewBuilder outlives the render pass that is used to draw egui.
    // (This restriction is likely to be address by Arcanization https://github.com/gfx-rs/wgpu/pull/3626).
    view_builder: ViewBuilderHandle,
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
        _egui_encoder: &mut wgpu::CommandEncoder,
        paint_callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(ctx) = paint_callback_resources.get_mut::<re_renderer::RenderContext>() else {
            re_log::error_once!(
                "Failed to execute egui prepare callback. No render context available."
            );
            return Vec::new();
        };

        // Takes the view_builder out of the slotmap, so we don't have a mutable reference to ctx in use.
        let Some(mut view_builder) = ctx
            .active_frame
            .per_frame_data_helper
            .get_mut::<ViewBuilderMap>()
            .and_then(|view_builder_map| {
                view_builder_map
                    .get_mut(self.view_builder)
                    .and_then(|slot| slot.take())
            })
        else {
            re_log::error_once!(
                "Failed to execute egui prepare callback. View builder with handle {:?} not found.",
                self.view_builder
            );
            return Vec::new();
        };

        let command_buffer = match view_builder.draw(ctx, self.clear_color) {
            Ok(command_buffer) => {
                // If drawing worked, put the view_builder back in so we can use it during paint.
                ctx.active_frame
                    .per_frame_data_helper
                    .get_mut::<ViewBuilderMap>()
                    .and_then(|view_builder_map| {
                        view_builder_map
                            .get_mut(self.view_builder)
                            .and_then(|slot| slot.replace(view_builder))
                    });
                vec![command_buffer]
            }

            Err(err) => {
                re_log::error_once!("Failed to fill view builder: {err}");
                // TODO(andreas): It would be nice to paint an error message instead.
                Vec::new()
            }
        };

        command_buffer
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
        // Future wgpu versions will lift this restriction and will allow us to remove this workaround.
        if ctx.active_frame.pinned_render_pipelines.is_none() {
            let render_pipelines = ctx.gpu_resources.render_pipelines.take_resources();
            ctx.active_frame.pinned_render_pipelines = Some(render_pipelines);
        }

        Vec::new()
    }

    fn paint<'a>(
        &'a self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        paint_callback_resources: &'a egui_wgpu::CallbackResources,
    ) {
        let Some(ctx) = paint_callback_resources.get::<re_renderer::RenderContext>() else {
            re_log::error_once!(
                "Failed to execute egui draw callback. No render context available."
            );
            return;
        };
        let Some(render_pipelines) = ctx.active_frame.pinned_render_pipelines.as_ref() else {
            re_log::error_once!(
                "Failed to execute egui draw callback. Render pipelines weren't transferred out of the pool first."
            );
            return;
        };

        let Some(Some(view_builder)) = ctx
            .active_frame
            .per_frame_data_helper
            .get::<ViewBuilderMap>()
            .and_then(|view_builder_map| view_builder_map.get(self.view_builder))
        else {
            re_log::error_once!(
                "Failed to execute egui draw callback. View builder with handle {:?} not found.",
                self.view_builder
            );
            return;
        };

        let screen_position = (info.viewport.min.to_vec2() * info.pixels_per_point).round();
        let screen_position = glam::vec2(screen_position.x, screen_position.y);

        view_builder.composite(ctx, render_pipelines, render_pass, screen_position);
    }
}
