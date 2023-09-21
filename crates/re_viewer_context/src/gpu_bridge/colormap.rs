use crate::gpu_bridge::{get_or_create_texture, render_image};

fn paint_colormap_gradient(
    render_ctx: &mut re_renderer::RenderContext,
    colormap: re_renderer::Colormap,
    painter: &egui::Painter,
    rect: egui::Rect,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let horizontal_gradient_id = egui::util::hash("horizontal_gradient");
    let horizontal_gradient = get_or_create_texture(render_ctx, horizontal_gradient_id, || {
        let width = 256;
        let height = 1;
        let data: Vec<u8> = (0..width)
            .flat_map(|x| {
                let t = x as f32 / (width as f32 - 1.0);
                half::f16::from_f32(t).to_le_bytes()
            })
            .collect();

        re_renderer::resource_managers::Texture2DCreationDesc {
            label: "horizontal_gradient".into(),
            data: data.into(),
            format: wgpu::TextureFormat::R16Float,
            width,
            height,
        }
    })
    .map_err(|err| anyhow::anyhow!("Failed to create horizontal gradient texture: {err}"))?;

    let colormapped_texture = re_renderer::renderer::ColormappedTexture {
        texture: horizontal_gradient,
        range: [0.0, 1.0],
        decode_srgb: false,
        multiply_rgb_with_alpha: false,
        gamma: 1.0,
        color_mapper: Some(re_renderer::renderer::ColorMapper::Function(colormap)),
    };

    let debug_name = format!("colormap_{colormap}");
    render_image(
        render_ctx,
        painter,
        rect,
        colormapped_texture,
        egui::TextureOptions::LINEAR,
        &debug_name,
    )
}

/// Show the given colormap as a horizontal bar.
fn colormap_preview_ui(
    render_ctx: &mut re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormap: re_renderer::Colormap,
) -> egui::Response {
    re_tracing::profile_function!();

    let desired_size = egui::vec2(128.0, 16.0);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    let rect = response.rect;

    if ui.is_rect_visible(rect) {
        if let Err(err) = paint_colormap_gradient(render_ctx, colormap, &painter, rect) {
            re_log::error_once!("Failed to paint colormap preview: {err}");
        }
    }

    response
}

pub fn colormap_dropdown_button_ui(
    render_ctx: &mut re_renderer::RenderContext,
    ui: &mut egui::Ui,
    map: &mut re_renderer::Colormap,
) {
    egui::ComboBox::from_id_source("color map select")
        .selected_text(map.to_string())
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);

            egui::Grid::new("colormap_selector")
                .num_columns(2)
                .show(ui, |ui| {
                    for option in re_renderer::Colormap::ALL {
                        ui.selectable_value(map, option, option.to_string());
                        colormap_preview_ui(render_ctx, ui, option);
                        ui.end_row();
                    }
                });
        });
}
