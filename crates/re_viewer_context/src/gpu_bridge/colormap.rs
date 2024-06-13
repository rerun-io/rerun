use crate::gpu_bridge::{get_or_create_texture, render_image};
use re_ui::list_item;

/// Show the given colormap as a horizontal bar.
fn colormap_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormap: re_types::components::Colormap,
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
        color_mapper: re_renderer::renderer::ColorMapper::Function(colormap_to_re_renderer(
            colormap,
        )),
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
    render_ctx: Option<&re_renderer::RenderContext>,
    ui: &mut egui::Ui,
    map: &mut re_types::components::Colormap,
) {
    let selected_text = map.to_string();
    let content_ui = |ui: &mut egui::Ui| {
        for option in re_types::components::Colormap::ALL {
            let list_item = list_item::ListItem::new().selected(&option == map);

            let response = if let Some(render_ctx) = render_ctx {
                list_item.show_flat(
                    ui,
                    list_item::PropertyContent::new(option.to_string()).value_fn(|ui, _| {
                        if let Err(err) = colormap_preview_ui(render_ctx, ui, option) {
                            re_log::error_once!("Failed to paint colormap preview: {err}");
                        }
                    }),
                )
            } else {
                list_item.show_flat(ui, list_item::LabelContent::new(option.to_string()))
            };

            if response.clicked() {
                *map = option;
            }
        }
    };

    egui::ComboBox::from_id_source("color map select")
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            list_item::list_item_scope(ui, "inner_scope", content_ui);
        });
}

pub fn colormap_to_re_renderer(colormap: re_types::components::Colormap) -> re_renderer::Colormap {
    match colormap {
        re_types::components::Colormap::Grayscale => re_renderer::Colormap::Grayscale,
        re_types::components::Colormap::Inferno => re_renderer::Colormap::Inferno,
        re_types::components::Colormap::Magma => re_renderer::Colormap::Magma,
        re_types::components::Colormap::Plasma => re_renderer::Colormap::Plasma,
        re_types::components::Colormap::Turbo => re_renderer::Colormap::Turbo,
        re_types::components::Colormap::Viridis => re_renderer::Colormap::Viridis,
    }
}
