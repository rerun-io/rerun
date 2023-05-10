use egui::{Color32, Vec2};
use itertools::Itertools as _;

use re_log_types::{
    component_types::{ClassId, Tensor, TensorDataMeaning},
    DecodedTensor, TensorElement,
};
use re_renderer::renderer::ColormappedTexture;
use re_ui::ReUi;
use re_viewer_context::{
    gpu_bridge, Annotations, TensorDecodeCache, TensorStats, TensorStatsCache, UiVerbosity,
    ViewerContext,
};

use super::EntityDataUi;

pub fn format_tensor_shape_single_line(
    shape: &[re_log_types::component_types::TensorDimension],
) -> String {
    format!("[{}]", shape.iter().join(", "))
}

impl EntityDataUi for Tensor {
    fn entity_data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        entity_path: &re_log_types::EntityPath,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        crate::profile_function!();

        match ctx.cache.entry::<TensorDecodeCache>().entry(self.clone()) {
            Ok(decoded) => {
                let annotations = crate::annotations(ctx, query, entity_path);
                tensor_ui(
                    ctx,
                    ui,
                    verbosity,
                    entity_path,
                    &annotations,
                    self,
                    &decoded,
                );
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        }
    }
}

fn tensor_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    verbosity: UiVerbosity,
    entity_path: &re_data_store::EntityPath,
    annotations: &Annotations,
    _encoded_tensor: &Tensor,
    tensor: &DecodedTensor,
) {
    // See if we can convert the tensor to a GPU texture.
    // Even if not, we will show info about the tensor.
    let tensor_stats = *ctx.cache.entry::<TensorStatsCache>().entry(tensor);
    let debug_name = entity_path.to_string();
    let texture_result = gpu_bridge::tensor_to_gpu(
        ctx.render_ctx,
        &debug_name,
        tensor,
        &tensor_stats,
        annotations,
    )
    .ok();

    match verbosity {
        UiVerbosity::Small => {
            ui.horizontal_centered(|ui| {
                if let Some(texture) = &texture_result {
                    let max_height = 24.0;
                    let max_size = Vec2::new(4.0 * max_height, max_height);
                    show_image_at_max_size(
                        ctx.render_ctx,
                        ctx.re_ui,
                        ui,
                        texture.clone(),
                        &debug_name,
                        max_size,
                    )
                    .on_hover_ui(|ui| {
                        // Show larger image on hover
                        let max_size = Vec2::splat(400.0);
                        show_image_at_max_size(
                            ctx.render_ctx,
                            ctx.re_ui,
                            ui,
                            texture.clone(),
                            &debug_name,
                            max_size,
                        );
                    });
                }

                ui.label(format!(
                    "{} x {}",
                    tensor.dtype(),
                    format_tensor_shape_single_line(tensor.shape())
                ))
                .on_hover_ui(|ui| tensor_summary_ui(ctx.re_ui, ui, tensor, &tensor_stats));
            });
        }

        UiVerbosity::All | UiVerbosity::Reduced => {
            ui.vertical(|ui| {
                ui.set_min_width(100.0);
                tensor_summary_ui(ctx.re_ui, ui, tensor, &tensor_stats);

                if let Some(texture) = &texture_result {
                    let max_size = ui
                        .available_size()
                        .min(texture_size(texture))
                        .min(egui::vec2(150.0, 300.0));
                    let response = show_image_at_max_size(
                        ctx.render_ctx,
                        ctx.re_ui,
                        ui,
                        texture.clone(),
                        &debug_name,
                        max_size,
                    );

                    if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                        let image_rect = response.rect;
                        show_zoomed_image_region_tooltip(
                            ctx.render_ctx,
                            ui,
                            response,
                            tensor,
                            &tensor_stats,
                            annotations,
                            tensor.meter,
                            &debug_name,
                            image_rect,
                            pointer_pos,
                        );
                    }

                    // TODO(emilk): support copying and saving images on web
                    #[cfg(not(target_arch = "wasm32"))]
                    if _encoded_tensor.data.is_compressed_image() || tensor.could_be_dynamic_image()
                    {
                        copy_and_save_image_ui(ui, tensor, _encoded_tensor);
                    }

                    if let Some([_h, _w, channels]) = tensor.image_height_width_channels() {
                        if channels == 3 {
                            if let re_log_types::component_types::TensorData::U8(data) =
                                &tensor.data
                            {
                                ui.collapsing("Histogram", |ui| {
                                    rgb8_histogram_ui(ui, data.as_slice());
                                });
                            }
                        }
                    }
                }
            });
        }
    }
}

fn texture_size(colormapped_texture: &ColormappedTexture) -> Vec2 {
    let [w, h] = colormapped_texture.texture.width_height();
    egui::vec2(w as f32, h as f32)
}

fn show_image_at_max_size(
    render_ctx: &mut re_renderer::RenderContext,
    re_ui: &ReUi,
    ui: &mut egui::Ui,
    colormapped_texture: ColormappedTexture,
    debug_name: &str,
    max_size: Vec2,
) -> egui::Response {
    let desired_size = {
        let mut desired_size = texture_size(&colormapped_texture);
        desired_size *= (max_size.x / desired_size.x).min(1.0);
        desired_size *= (max_size.y / desired_size.y).min(1.0);
        desired_size
    };

    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    if let Err(err) = gpu_bridge::render_image(
        render_ctx,
        &painter,
        response.rect,
        colormapped_texture,
        egui::TextureOptions::LINEAR,
        debug_name,
    ) {
        let label_response = ui.label(re_ui.error_text(err.to_string()));
        response.union(label_response)
    } else {
        response
    }
}

pub fn tensor_summary_ui_grid_contents(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
) {
    let Tensor {
        tensor_id: _,
        shape,
        data,
        meaning,
        meter,
    } = tensor;

    re_ui
        .grid_left_hand_label(ui, "Data type")
        .on_hover_text("Data type used for all individual elements within the tensor.");
    ui.label(tensor.dtype().to_string());
    ui.end_row();

    re_ui
        .grid_left_hand_label(ui, "Shape")
        .on_hover_text("Extent of every dimension.");
    ui.vertical(|ui| {
        // For unnamed tensor dimension more than a single line usually doesn't make sense!
        // But what if some are named and some are not?
        // -> If more than 1 is named, make it a column!
        if shape.iter().filter(|d| d.name.is_some()).count() > 1 {
            for dim in shape {
                ui.label(dim.to_string());
            }
        } else {
            ui.label(format_tensor_shape_single_line(shape));
        }
    });
    ui.end_row();

    if *meaning != TensorDataMeaning::Unknown {
        re_ui.grid_left_hand_label(ui, "Meaning");
        ui.label(match meaning {
            TensorDataMeaning::Unknown => "",
            TensorDataMeaning::ClassId => "Class ID",
            TensorDataMeaning::Depth => "Depth",
        });
        ui.end_row();
    }

    if let Some(meter) = meter {
        re_ui
            .grid_left_hand_label(ui, "Meter")
            .on_hover_text(format!("{meter} depth units equals one world unit"));
        ui.label(meter.to_string());
        ui.end_row();
    }

    match data {
        re_log_types::component_types::TensorData::U8(_)
        | re_log_types::component_types::TensorData::U16(_)
        | re_log_types::component_types::TensorData::U32(_)
        | re_log_types::component_types::TensorData::U64(_)
        | re_log_types::component_types::TensorData::I8(_)
        | re_log_types::component_types::TensorData::I16(_)
        | re_log_types::component_types::TensorData::I32(_)
        | re_log_types::component_types::TensorData::I64(_)
        | re_log_types::component_types::TensorData::F16(_)
        | re_log_types::component_types::TensorData::F32(_)
        | re_log_types::component_types::TensorData::F64(_) => {}
        re_log_types::component_types::TensorData::JPEG(jpeg_bytes) => {
            re_ui.grid_left_hand_label(ui, "Encoding");
            ui.label(format!(
                "{} JPEG",
                re_format::format_bytes(jpeg_bytes.len() as _),
            ));
            ui.end_row();
        }
    }

    let TensorStats { range } = tensor_stats;

    if let Some((min, max)) = range {
        ui.label("Data range")
            .on_hover_text("All values of the tensor range within these bounds.");
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
}

pub fn tensor_summary_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
) {
    egui::Grid::new("tensor_summary_ui")
        .num_columns(2)
        .show(ui, |ui| {
            tensor_summary_ui_grid_contents(re_ui, ui, tensor, tensor_stats);
        });
}

#[allow(clippy::too_many_arguments)]
fn show_zoomed_image_region_tooltip(
    render_ctx: &mut re_renderer::RenderContext,
    parent_ui: &mut egui::Ui,
    response: egui::Response,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
) -> egui::Response {
    response
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(320.0);
            ui.horizontal(|ui| {
                if let Some([h, w, _]) = tensor.image_height_width_channels() {
                    use egui::remap_clamp;

                    let center_texel = [
                        (remap_clamp(pointer_pos.x, image_rect.x_range(), 0.0..=w as f32) as isize),
                        (remap_clamp(pointer_pos.y, image_rect.y_range(), 0.0..=h as f32) as isize),
                    ];
                    show_zoomed_image_region_area_outline(
                        parent_ui,
                        tensor,
                        center_texel,
                        image_rect,
                    );
                    show_zoomed_image_region(
                        render_ctx,
                        ui,
                        tensor,
                        tensor_stats,
                        annotations,
                        meter,
                        debug_name,
                        center_texel,
                    );
                }
            });
        })
}

// Show the surrounding pixels:
const ZOOMED_IMAGE_TEXEL_RADIUS: isize = 10;

pub fn show_zoomed_image_region_area_outline(
    ui: &mut egui::Ui,
    tensor: &Tensor,
    [center_x, center_y]: [isize; 2],
    image_rect: egui::Rect,
) {
    use egui::{pos2, remap, Rect};

    let Some([height, width, _]) = tensor.image_height_width_channels() else {return;};

    let width = width as f32;
    let height = height as f32;

    // Show where on the original image the zoomed-in region is at:
    let left = (center_x - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let right = (center_x + ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let top = (center_y - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let bottom = (center_y + ZOOMED_IMAGE_TEXEL_RADIUS) as f32;

    let left = remap(left, 0.0..=width, image_rect.x_range());
    let right = remap(right, 0.0..=width, image_rect.x_range());
    let top = remap(top, 0.0..=height, image_rect.y_range());
    let bottom = remap(bottom, 0.0..=height, image_rect.y_range());

    let rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
    let painter = ui.ctx().debug_painter();
    painter.rect_stroke(rect, 0.0, (2.0, Color32::BLACK));
    painter.rect_stroke(rect, 0.0, (1.0, Color32::WHITE));
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
pub fn show_zoomed_image_region(
    render_ctx: &mut re_renderer::RenderContext,
    ui: &mut egui::Ui,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
) {
    if let Err(err) = try_show_zoomed_image_region(
        render_ctx,
        ui,
        tensor,
        tensor_stats,
        annotations,
        meter,
        debug_name,
        center_texel,
    ) {
        ui.label(format!("Error: {err}"));
    }
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
fn try_show_zoomed_image_region(
    render_ctx: &mut re_renderer::RenderContext,
    ui: &mut egui::Ui,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
) -> anyhow::Result<()> {
    let texture =
        gpu_bridge::tensor_to_gpu(render_ctx, debug_name, tensor, tensor_stats, annotations)?;

    let Some([height, width, _]) = tensor.image_height_width_channels() else { return Ok(()); };

    const POINTS_PER_TEXEL: f32 = 5.0;
    let size = Vec2::splat((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32 * POINTS_PER_TEXEL);

    let (_id, zoom_rect) = ui.allocate_space(size);
    let painter = ui.painter();

    painter.rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    {
        let image_rect_on_screen = egui::Rect::from_min_size(
            zoom_rect.center()
                - POINTS_PER_TEXEL
                    * egui::vec2(center_texel[0] as f32 + 0.5, center_texel[1] as f32 + 0.5),
            POINTS_PER_TEXEL * egui::vec2(width as f32, height as f32),
        );

        gpu_bridge::render_image(
            render_ctx,
            &painter.with_clip_rect(zoom_rect),
            image_rect_on_screen,
            texture.clone(),
            egui::TextureOptions::NEAREST,
            debug_name,
        )?;
    }

    // Show the center text, to indicate which texel we're printing the values of:
    {
        let center_texel_rect =
            egui::Rect::from_center_size(zoom_rect.center(), Vec2::splat(POINTS_PER_TEXEL));
        painter.rect_stroke(center_texel_rect.expand(1.0), 0.0, (1.0, Color32::BLACK));
        painter.rect_stroke(center_texel_rect, 0.0, (1.0, Color32::WHITE));
    }

    let [x, y] = center_texel;
    if 0 <= x && (x as u64) < width && 0 <= y && (y as u64) < height {
        ui.separator();

        ui.vertical(|ui| {
            tensor_pixel_value_ui(ui, tensor, annotations, [x as _, y as _], meter);

            // Show a big sample of the color of the middle texel:
            let (rect, _) =
                ui.allocate_exact_size(Vec2::splat(ui.available_height()), egui::Sense::hover());
            // Position texture so that the center texel is at the center of the rect:
            let zoom = rect.width();
            let image_rect_on_screen = egui::Rect::from_min_size(
                rect.center()
                    - zoom * egui::vec2(center_texel[0] as f32 + 0.5, center_texel[1] as f32 + 0.5),
                zoom * egui::vec2(width as f32, height as f32),
            );
            gpu_bridge::render_image(
                render_ctx,
                &ui.painter().with_clip_rect(rect),
                image_rect_on_screen,
                texture,
                egui::TextureOptions::NEAREST,
                debug_name,
            )
        })
        .inner?;
    }
    Ok(())
}

fn tensor_pixel_value_ui(
    ui: &mut egui::Ui,
    tensor: &Tensor,
    annotations: &Annotations,
    [x, y]: [u64; 2],
    meter: Option<f32>,
) {
    egui::Grid::new("hovered pixel properties").show(ui, |ui| {
        ui.label("Position:");
        ui.label(format!("{x}, {y}"));
        ui.end_row();

        if tensor.num_dim() == 2 {
            if let Some(raw_value) = tensor.get(&[y, x]) {
                if let (TensorDataMeaning::ClassId, Some(u16_val)) =
                    (tensor.meaning(), raw_value.try_as_u16())
                {
                    ui.label("Label:");
                    ui.label(
                        annotations
                            .class_description(Some(ClassId(u16_val)))
                            .annotation_info()
                            .label(None)
                            .unwrap_or_else(|| u16_val.to_string()),
                    );
                    ui.end_row();
                };
            }
        }
        if let Some(meter) = meter {
            // This is a depth map
            if let Some(raw_value) = tensor.get(&[y, x]) {
                let raw_value = raw_value.as_f64();
                let meters = raw_value / meter as f64;
                ui.label("Depth:");
                if meters < 1.0 {
                    ui.monospace(format!("{:.1} mm", meters * 1e3));
                } else {
                    ui.monospace(format!("{meters:.3} m"));
                }
            }
        }
    });

    let text = if let Some([_, _, channel]) = tensor.image_height_width_channels() {
        match channel {
            1 => tensor
                .get_with_image_coords(x, y, 0)
                .map(|v| format!("Val: {v}")),
            3 => {
                // TODO(jleibs): Track RGB ordering somehow -- don't just assume it
                if let (Some(r), Some(g), Some(b)) = (
                    tensor.get_with_image_coords(x, y, 0),
                    tensor.get_with_image_coords(x, y, 1),
                    tensor.get_with_image_coords(x, y, 2),
                ) {
                    match (r, g, b) {
                        (TensorElement::U8(r), TensorElement::U8(g), TensorElement::U8(b)) => {
                            Some(format!("R: {r}, G: {g}, B: {b}, #{r:02X}{g:02X}{b:02X}"))
                        }
                        _ => Some(format!("R: {r}, G: {g}, B: {b}")),
                    }
                } else {
                    None
                }
            }
            4 => {
                // TODO(jleibs): Track RGB ordering somehow -- don't just assume it
                if let (Some(r), Some(g), Some(b), Some(a)) = (
                    tensor.get_with_image_coords(x, y, 0),
                    tensor.get_with_image_coords(x, y, 1),
                    tensor.get_with_image_coords(x, y, 2),
                    tensor.get_with_image_coords(x, y, 3),
                ) {
                    match (r, g, b, a) {
                        (
                            TensorElement::U8(r),
                            TensorElement::U8(g),
                            TensorElement::U8(b),
                            TensorElement::U8(a),
                        ) => Some(format!(
                            "R: {r}, G: {g}, B: {b}, A: {a}, #{r:02X}{g:02X}{b:02X}{a:02X}"
                        )),
                        _ => Some(format!("R: {r}, G: {g}, B: {b}, A: {a}")),
                    }
                } else {
                    None
                }
            }
            channel => Some(format!("Cannot preview {channel}-size channel image")),
        }
    } else {
        Some(format!(
            "Cannot preview tensors with a shape of {:?}",
            tensor.shape()
        ))
    };

    if let Some(text) = text {
        ui.label(text);
    } else {
        ui.label("No Value");
    }
}

fn rgb8_histogram_ui(ui: &mut egui::Ui, rgb: &[u8]) -> egui::Response {
    crate::profile_function!();

    let mut histograms = [[0_u64; 256]; 3];
    {
        // TODO(emilk): this is slow, so cache the results!
        crate::profile_scope!("build");
        for pixel in rgb.chunks_exact(3) {
            for c in 0..3 {
                histograms[c][pixel[c] as usize] += 1;
            }
        }
    }

    use egui::plot::{Bar, BarChart, Legend, Plot};

    let names = ["R", "G", "B"];
    let colors = [Color32::RED, Color32::GREEN, Color32::BLUE];

    let charts = histograms
        .into_iter()
        .enumerate()
        .map(|(component, histogram)| {
            let fill = colors[component].linear_multiply(0.5);

            BarChart::new(
                histogram
                    .into_iter()
                    .enumerate()
                    .map(|(i, count)| {
                        Bar::new(i as _, count as _)
                            .width(0.9)
                            .fill(fill)
                            .vertical()
                            .stroke(egui::Stroke::NONE)
                    })
                    .collect(),
            )
            .color(colors[component])
            .name(names[component])
        })
        .collect_vec();

    crate::profile_scope!("show");
    Plot::new("rgb_histogram")
        .legend(Legend::default())
        .height(200.0)
        .show_axes([false; 2])
        .show(ui, |plot_ui| {
            for chart in charts {
                plot_ui.bar_chart(chart);
            }
        })
        .response
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_and_save_image_ui(ui: &mut egui::Ui, tensor: &Tensor, _encoded_tensor: &Tensor) {
    ui.horizontal(|ui| {
        if tensor.could_be_dynamic_image() && ui.button("Click to copy image").clicked() {
            match tensor.to_dynamic_image() {
                Ok(dynamic_image) => {
                    let rgba = dynamic_image.to_rgba8();
                    re_viewer_context::Clipboard::with(|clipboard| {
                        clipboard.set_image(
                            [rgba.width() as _, rgba.height() as _],
                            bytemuck::cast_slice(rgba.as_raw()),
                        );
                    });
                }
                Err(err) => {
                    re_log::error!("Failed to convert tensor to image: {err}");
                }
            }
        }

        if ui.button("Save image…").clicked() {
            match tensor.to_dynamic_image() {
                Ok(dynamic_image) => {
                    save_image(_encoded_tensor, &dynamic_image);
                }
                Err(err) => {
                    re_log::error!("Failed to convert tensor to image: {err}");
                }
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn save_image(tensor: &re_log_types::component_types::Tensor, dynamic_image: &image::DynamicImage) {
    use re_log_types::component_types::TensorData;

    match &tensor.data {
        TensorData::JPEG(bytes) => {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("image.jpg")
                .save_file()
            {
                match write_binary(&path, bytes.as_slice()) {
                    Ok(()) => {
                        re_log::info!("Image saved to {path:?}");
                    }
                    Err(err) => {
                        re_log::error!(
                            "Failed saving image to {path:?}: {}",
                            re_error::format(&err)
                        );
                    }
                }
            }
        }
        _ => {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("image.png")
                .save_file()
            {
                match dynamic_image.save(&path) {
                    // TODO(emilk): show a popup instead of logging result
                    Ok(()) => {
                        re_log::info!("Image saved to {path:?}");
                    }
                    Err(err) => {
                        re_log::error!("Failed saving image to {path:?}: {err}");
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_binary(path: &std::path::PathBuf, data: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;
    Ok(std::fs::File::create(path)?.write_all(data)?)
}
