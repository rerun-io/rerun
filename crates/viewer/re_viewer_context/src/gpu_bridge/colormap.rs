use re_sdk_types::ColormapCategory;
use re_sdk_types::reflection::Enum as _;
use re_ui::list_item;

use crate::MaybeMutRef;
use crate::gpu_bridge::{get_or_create_texture, render_image};

const MIN_WIDTH: f32 = 150.0;

/// Show the given colormap as a horizontal bar.
fn colormap_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormap: re_sdk_types::components::Colormap,
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

        re_renderer::resource_managers::ImageDataDesc {
            label: "horizontal_gradient".into(),
            data: data.into(),
            format: wgpu::TextureFormat::R16Float.into(),
            width_height: [width, height],
            alpha_channel_usage: re_renderer::AlphaChannelUsage::Opaque,
        }
    })
    .map_err(|err| anyhow::anyhow!("Failed to create horizontal gradient texture: {err}"))?;

    let colormapped_texture = re_renderer::renderer::ColormappedTexture {
        texture: horizontal_gradient,
        range: [0.0, 1.0],
        decode_srgb: false,
        texture_alpha: re_renderer::renderer::TextureAlpha::Opaque,
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
        debug_name.into(),
    )?;

    Ok(response)
}

fn colormap_variant_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    option: &re_sdk_types::components::Colormap,
    map: &mut re_sdk_types::components::Colormap,
) -> egui::Response {
    let list_item = list_item::ListItem::new().selected(option == map);

    let mut response = list_item.show_flat(
        ui,
        list_item::CustomContent::new(|ui, _| {
            if let Err(err) = colormap_preview_ui(render_ctx, ui, *option) {
                re_log::error_once!("Failed to paint colormap preview: {err}");
            }

            ui.add_space(8.0);

            ui.label(option.to_string());
        }),
    );

    if response.clicked() {
        *map = *option;
        response.mark_changed();
    }

    response
}

fn colormap_category_ui(
    ctx: &crate::ViewerContext<'_>,
    ui: &mut egui::Ui,
    category: ColormapCategory,
    selected: &mut re_sdk_types::components::Colormap,
) -> egui::Response {
    let label_content = match category {
        ColormapCategory::Sequential => "Sequential",
        ColormapCategory::Diverging => "Diverging",
        ColormapCategory::Cyclic => "Cyclic",
    };

    let mut response = list_item::ListItem::new()
        .interactive(false)
        .header()
        .show_flat(
            ui,
            list_item::LabelContent::header(label_content)
                .strong(true)
                .min_desired_width(MIN_WIDTH),
        );

    for option in re_sdk_types::components::Colormap::variants()
        .iter()
        .filter(|&&colormap| colormap.category() == category)
    {
        response |= colormap_variant_ui(ctx.render_ctx(), ui, option, selected);
    }

    response
}

pub fn colormap_edit_or_view_ui(
    ctx: &crate::ViewerContext<'_>,
    ui: &mut egui::Ui,
    map: &mut MaybeMutRef<'_, re_sdk_types::components::Colormap>,
) -> egui::Response {
    if let Some(map) = map.as_mut() {
        let selected_text = map.to_string();
        let content_ui = |ui: &mut egui::Ui| {
            let mut response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());

            response |= colormap_category_ui(ctx, ui, ColormapCategory::Sequential, map);
            response |= colormap_category_ui(ctx, ui, ColormapCategory::Diverging, map);
            response |= colormap_category_ui(ctx, ui, ColormapCategory::Cyclic, map);

            response
        };

        let mut inner_response = egui::ComboBox::from_id_salt("color map select")
            .selected_text(selected_text)
            .height(400.0)
            .show_ui(ui, |ui| {
                list_item::list_item_scope(ui, "inner_scope", content_ui)
            });
        if let Some(response) = inner_response.inner
            && response.inner.changed()
        {
            inner_response.response.mark_changed();
        }
        inner_response.response
    } else {
        let map: re_sdk_types::components::Colormap = **map;
        let colormap_response = {
            let result = colormap_preview_ui(ctx.render_ctx(), ui, map);
            if let Err(err) = &result {
                re_log::error_once!("Failed to paint colormap preview: {err}");
            }
            result.ok()
        };

        let label_response = ui.add(egui::Label::new(map.to_string()).truncate());

        match colormap_response {
            Some(colormap_response) => colormap_response | label_response,
            None => label_response,
        }
    }
}

pub fn colormap_to_re_renderer(
    colormap: re_sdk_types::components::Colormap,
) -> re_renderer::Colormap {
    match colormap {
        re_sdk_types::components::Colormap::Grayscale => re_renderer::Colormap::Grayscale,
        re_sdk_types::components::Colormap::Inferno => re_renderer::Colormap::Inferno,
        re_sdk_types::components::Colormap::Magma => re_renderer::Colormap::Magma,
        re_sdk_types::components::Colormap::Plasma => re_renderer::Colormap::Plasma,
        re_sdk_types::components::Colormap::Turbo => re_renderer::Colormap::Turbo,
        re_sdk_types::components::Colormap::Viridis => re_renderer::Colormap::Viridis,
        re_sdk_types::components::Colormap::CyanToYellow => re_renderer::Colormap::CyanToYellow,
        re_sdk_types::components::Colormap::Spectral => re_renderer::Colormap::Spectral,
        re_sdk_types::components::Colormap::Twilight => re_renderer::Colormap::Twilight,
    }
}
