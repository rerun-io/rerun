use crate::gpu_bridge::{get_or_create_texture, render_image};

/// Show the given colormap as a horizontal bar.
fn colormap_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormap: re_renderer::Colormap,
) -> anyhow::Result<egui::Response> {
    re_tracing::profile_function!();

    let desired_size = egui::vec2(128.0, 16.0);
    let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return Ok(response);
    }

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
        shader_decoding: None,
        color_mapper: re_renderer::renderer::ColorMapper::Function(colormap),
    };

    let debug_name = format!("colormap_{colormap}");
    render_image(
        render_ctx,
        ui.painter(),
        rect,
        colormapped_texture,
        egui::TextureOptions::LINEAR,
        &debug_name,
    )?;

    Ok(response)
}

pub fn colormap_dropdown_button_ui(
    render_ctx: &re_renderer::RenderContext,
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
                        if let Err(err) = colormap_preview_ui(render_ctx, ui, option) {
                            re_log::error_once!("Failed to paint colormap preview: {err}");
                        }
                        ui.end_row();
                    }
                });
        });
}
