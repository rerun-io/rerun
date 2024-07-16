use egui::{Color32, Vec2};
use itertools::Itertools as _;

use re_chunk_store::RowId;
use re_log_types::EntityPath;
use re_renderer::renderer::ColormappedTexture;
use re_types::components::{ClassId, Colormap, DepthMeter};
use re_types::datatypes::{TensorBuffer, TensorData, TensorDimension};
use re_types::tensor_data::{TensorDataMeaning, TensorElement};
use re_ui::UiExt as _;
use re_viewer_context::{
    gpu_bridge, Annotations, TensorStats, TensorStatsCache, UiLayout, ViewerContext,
};

use crate::image_meaning::image_meaning_for_entity;

use super::EntityDataUi;

pub fn format_tensor_shape_single_line(shape: &[TensorDimension]) -> String {
    const MAX_SHOWN: usize = 4; // should be enough for width/height/depth and then some!
    let iter = shape.iter().take(MAX_SHOWN);
    let labelled = iter.clone().any(|dim| dim.name.is_some());
    let shapes = iter
        .map(|dim| {
            format!(
                "{}{}",
                dim.size,
                if let Some(name) = &dim.name {
                    format!(" ({name})")
                } else {
                    String::new()
                }
            )
        })
        .join(if labelled { " × " } else { "×" });
    format!(
        "{shapes}{}",
        if shape.len() > MAX_SHOWN {
            if labelled {
                " × …"
            } else {
                "×…"
            }
        } else {
            ""
        }
    )
}

impl EntityDataUi for re_types::components::TensorData {
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();

        // TODO(#5607): what should happen if the promise is still pending?
        let tensor_data_row_id = ctx
            .recording()
            .latest_at_component::<Self>(entity_path, query)
            .map_or(RowId::ZERO, |tensor| tensor.index.1);

        let annotations = crate::annotations(ctx, query, entity_path);
        tensor_ui(
            ctx,
            query,
            db,
            ui,
            ui_layout,
            entity_path,
            &annotations,
            tensor_data_row_id,
            &self.0,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn tensor_ui(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    entity_path: &re_entity_db::EntityPath,
    annotations: &Annotations,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
) {
    // See if we can convert the tensor to a GPU texture.
    // Even if not, we will show info about the tensor.
    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));
    let debug_name = entity_path.to_string();

    let meaning = image_meaning_for_entity(entity_path, query, db.store());

    let (meter, colormap) = if meaning == TensorDataMeaning::Depth {
        // TODO(#5607): what should happen if the promise is still pending?
        (
            ctx.recording()
                .latest_at_component::<DepthMeter>(entity_path, query)
                .map(|meter| *meter.value.0),
            ctx.recording()
                .latest_at_component::<Colormap>(entity_path, query)
                .map(|colormap| colormap.value),
        )
    } else {
        (None, None)
    };

    let Some(render_ctx) = ctx.render_ctx else {
        return;
    };

    let texture_result = gpu_bridge::tensor_to_gpu(
        render_ctx,
        &debug_name,
        tensor_data_row_id,
        tensor,
        meaning,
        &tensor_stats,
        annotations,
        colormap,
    )
    .ok();

    match ui_layout {
        UiLayout::List => {
            ui.horizontal(|ui| {
                if let Some(texture) = &texture_result {
                    // We want all preview images to take up the same amount of space,
                    // no matter what the actual aspect ratio of the images are.
                    let preview_size = Vec2::splat(ui.available_height());
                    ui.allocate_ui_with_layout(
                        preview_size,
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            ui.set_min_size(preview_size);

                            match show_image_preview(
                                render_ctx,
                                ui,
                                texture.clone(),
                                &debug_name,
                                preview_size,
                            ) {
                                Ok(response) => response.on_hover_ui(|ui| {
                                    // Show larger image on hover.
                                    let preview_size = Vec2::splat(400.0);
                                    show_image_preview(
                                        render_ctx,
                                        ui,
                                        texture.clone(),
                                        &debug_name,
                                        preview_size,
                                    )
                                    .ok();
                                }),
                                Err((response, err)) => response.on_hover_text(err.to_string()),
                            }
                        },
                    );
                }

                let shape = match tensor.image_height_width_channels() {
                    Some([h, w, c]) => vec![
                        TensorDimension::height(h),
                        TensorDimension::width(w),
                        TensorDimension::depth(c),
                    ],
                    None => tensor.shape.clone(),
                };
                let text = format!(
                    "{}, {}",
                    tensor.dtype(),
                    format_tensor_shape_single_line(&shape)
                );
                ui_layout.label(ui, text).on_hover_ui(|ui| {
                    tensor_summary_ui(ui, tensor, meaning, meter, &tensor_stats);
                });
            });
        }

        UiLayout::SelectionPanelFull | UiLayout::SelectionPanelLimitHeight | UiLayout::Tooltip => {
            ui.vertical(|ui| {
                ui.set_min_width(100.0);
                tensor_summary_ui(ui, tensor, meaning, meter, &tensor_stats);

                if let Some(texture) = &texture_result {
                    let preview_size = ui
                        .available_size()
                        .min(texture_size(texture))
                        .min(egui::vec2(150.0, 300.0));
                    let response = match show_image_preview(
                        render_ctx,
                        ui,
                        texture.clone(),
                        &debug_name,
                        preview_size,
                    ) {
                        Ok(response) => response,
                        Err((response, err)) => response.on_hover_text(err.to_string()),
                    };

                    if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                        let image_rect = response.rect;
                        show_zoomed_image_region_tooltip(
                            render_ctx,
                            ui,
                            response,
                            tensor_data_row_id,
                            tensor,
                            &tensor_stats,
                            annotations,
                            meaning,
                            meter,
                            &debug_name,
                            image_rect,
                            pointer_pos,
                            colormap,
                        );
                    }

                    // TODO(emilk): support copying and saving images on web
                    #[cfg(not(target_arch = "wasm32"))]
                    if tensor.buffer.is_compressed_image() || tensor.could_be_dynamic_image() {
                        copy_and_save_image_ui(ui, tensor, tensor);
                    }

                    if let Some([_h, _w, channels]) = tensor.image_height_width_channels() {
                        if let TensorBuffer::Nv12(_) = &tensor.buffer {
                            return;
                        }
                        if channels == 3 {
                            if let TensorBuffer::U8(data) = &tensor.buffer {
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
    let [w, h] = colormapped_texture.width_height();
    egui::vec2(w as f32, h as f32)
}

/// Shows preview of an image.
///
/// Displays the image at the desired size, without overshooting it, and preserving aspect ration.
///
/// Extremely small images will be stretched on their thin axis to make them visible.
/// This does not preserve aspect ratio, but we only stretch it to a very thin size, so it is fine.
///
/// Returns error if the image could not be rendered.
fn show_image_preview(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormapped_texture: ColormappedTexture,
    debug_name: &str,
    desired_size: egui::Vec2,
) -> Result<egui::Response, (egui::Response, anyhow::Error)> {
    const MIN_SIZE: f32 = 2.0;

    let texture_size = texture_size(&colormapped_texture);

    let scaled_size = largest_size_that_fits_in(texture_size.x / texture_size.y, desired_size);

    // Don't allow images so thin that we cannot see them:
    let scaled_size = scaled_size.max(Vec2::splat(MIN_SIZE));

    let (response, painter) = ui.allocate_painter(scaled_size, egui::Sense::hover());

    // Place it in the center:
    let texture_rect_on_screen = egui::Rect::from_center_size(response.rect.center(), scaled_size);

    if let Err(err) = gpu_bridge::render_image(
        render_ctx,
        &painter,
        texture_rect_on_screen,
        colormapped_texture,
        egui::TextureOptions::LINEAR,
        debug_name,
    ) {
        let color = ui.visuals().error_fg_color;
        painter.text(
            response.rect.left_top(),
            egui::Align2::LEFT_TOP,
            "🚫",
            egui::FontId::default(),
            color,
        );
        Err((response, err))
    } else {
        Ok(response)
    }
}

fn largest_size_that_fits_in(aspect_ratio: f32, max_size: Vec2) -> Vec2 {
    if aspect_ratio < max_size.x / max_size.y {
        // A thin image in a landscape frame
        egui::vec2(aspect_ratio * max_size.y, max_size.y)
    } else {
        // A wide image in a portrait frame
        egui::vec2(max_size.x, max_size.x / aspect_ratio)
    }
}

pub fn tensor_summary_ui_grid_contents(
    ui: &mut egui::Ui,
    tensor: &TensorData,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
    tensor_stats: &TensorStats,
) {
    let TensorData { shape, buffer: _ } = tensor;

    ui.grid_left_hand_label("Data type")
        .on_hover_text("Data type used for all individual elements within the tensor");
    ui.label(tensor.dtype().to_string());
    ui.end_row();

    ui.grid_left_hand_label("Shape")
        .on_hover_text("Extent of every dimension");
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

    if meaning != TensorDataMeaning::Unknown {
        ui.grid_left_hand_label("Meaning");
        ui.label(match meaning {
            TensorDataMeaning::Unknown => "",
            TensorDataMeaning::ClassId => "Class ID",
            TensorDataMeaning::Depth => "Depth",
        });
        ui.end_row();
    }

    if let Some(meter) = meter {
        ui.grid_left_hand_label("Meter")
            .on_hover_text(format!("{meter} depth units equals one world unit"));
        ui.label(meter.to_string());
        ui.end_row();
    }

    match &tensor.buffer {
        TensorBuffer::U8(_)
        | TensorBuffer::U16(_)
        | TensorBuffer::U32(_)
        | TensorBuffer::U64(_)
        | TensorBuffer::I8(_)
        | TensorBuffer::I16(_)
        | TensorBuffer::I32(_)
        | TensorBuffer::I64(_)
        | TensorBuffer::F16(_)
        | TensorBuffer::F32(_)
        | TensorBuffer::F64(_) => {}

        TensorBuffer::Nv12(_) => {
            ui.grid_left_hand_label("Encoding");
            ui.label("NV12");
            ui.end_row();
        }

        TensorBuffer::Yuy2(_) => {
            ui.grid_left_hand_label("Encoding");
            ui.label("YUY2");
            ui.end_row();
        }
    }

    let TensorStats {
        range,
        finite_range,
    } = tensor_stats;

    if let Some((min, max)) = range {
        ui.label("Data range")
            .on_hover_text("All values of the tensor range within these bounds");
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
    // Show finite range only if it is different from the actual range.
    if let (true, Some((min, max))) = (range != finite_range, finite_range) {
        ui.label("Finite data range").on_hover_text(
            "The finite values (ignoring all NaN & -Inf/+Inf) of the tensor range within these bounds"
        );
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
}

pub fn tensor_summary_ui(
    ui: &mut egui::Ui,
    tensor: &TensorData,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
    tensor_stats: &TensorStats,
) {
    egui::Grid::new("tensor_summary_ui")
        .num_columns(2)
        .show(ui, |ui| {
            tensor_summary_ui_grid_contents(ui, tensor, meaning, meter, tensor_stats);
        });
}

#[allow(clippy::too_many_arguments)]
fn show_zoomed_image_region_tooltip(
    render_ctx: &re_renderer::RenderContext,
    parent_ui: &egui::Ui,
    response: egui::Response,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
    debug_name: &str,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    colormap: Option<Colormap>,
) -> egui::Response {
    let response_rect = response.rect;
    response
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(320.0);
            ui.horizontal(|ui| {
                if let Some([h, w, _]) = tensor.image_height_width_channels() {
                    use egui::remap_clamp;

                    let center_texel = [
                        remap_clamp(pointer_pos.x, image_rect.x_range(), 0.0..=w as f32) as isize,
                        remap_clamp(pointer_pos.y, image_rect.y_range(), 0.0..=h as f32) as isize,
                    ];
                    show_zoomed_image_region_area_outline(
                        parent_ui.ctx(),
                        response_rect,
                        egui::vec2(w as f32, h as f32),
                        center_texel,
                        image_rect,
                    );
                    show_zoomed_image_region(
                        render_ctx,
                        ui,
                        tensor_data_row_id,
                        tensor,
                        tensor_stats,
                        annotations,
                        meaning,
                        meter,
                        debug_name,
                        center_texel,
                        colormap,
                    );
                }
            });
        })
}

// Show the surrounding pixels:
const ZOOMED_IMAGE_TEXEL_RADIUS: isize = 10;

pub fn show_zoomed_image_region_area_outline(
    egui_ctx: &egui::Context,
    ui_clip_rect: egui::Rect,
    image_resolution: egui::Vec2,
    [center_x, center_y]: [isize; 2],
    image_rect: egui::Rect,
) {
    use egui::{pos2, remap, Rect};

    let width = image_resolution.x;
    let height = image_resolution.y;

    // Show where on the original image the zoomed-in region is at:
    // The area shown is ZOOMED_IMAGE_TEXEL_RADIUS _surrounding_ the center.
    // Since the center is the top-left/rounded-down, coordinate, we need to add 1 to right/bottom.
    let left = (center_x - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let right = (center_x + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;
    let top = (center_y - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let bottom = (center_y + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;

    let left = remap(left, 0.0..=width, image_rect.x_range());
    let right = remap(right, 0.0..=width, image_rect.x_range());
    let top = remap(top, 0.0..=height, image_rect.y_range());
    let bottom = remap(bottom, 0.0..=height, image_rect.y_range());

    let sample_rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
    let painter = egui_ctx.debug_painter().with_clip_rect(ui_clip_rect);
    painter.rect_stroke(sample_rect, 0.0, (2.0, Color32::BLACK));
    painter.rect_stroke(sample_rect, 0.0, (1.0, Color32::WHITE));
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
pub fn show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
    colormap: Option<Colormap>,
) {
    if let Err(err) = try_show_zoomed_image_region(
        render_ctx,
        ui,
        tensor_data_row_id,
        tensor,
        tensor_stats,
        annotations,
        meaning,
        meter,
        debug_name,
        center_texel,
        colormap,
    ) {
        ui.label(format!("Error: {err}"));
    }
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
fn try_show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
    colormap: Option<Colormap>,
) -> anyhow::Result<()> {
    let Some([height, width, _]) = tensor.image_height_width_channels() else {
        return Ok(());
    };

    let texture = gpu_bridge::tensor_to_gpu(
        render_ctx,
        debug_name,
        tensor_data_row_id,
        tensor,
        meaning,
        tensor_stats,
        annotations,
        colormap,
    )?;

    const POINTS_PER_TEXEL: f32 = 5.0;
    let size = Vec2::splat(((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32) * POINTS_PER_TEXEL);

    let (_id, zoom_rect) = ui.allocate_space(size);
    let painter = ui.painter();

    painter.rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    // Paint the zoomed in region:
    {
        let image_rect_on_screen = egui::Rect::from_min_size(
            zoom_rect.center()
                - POINTS_PER_TEXEL
                    * egui::vec2(
                        (center_texel[0] as f32) + 0.5,
                        (center_texel[1] as f32) + 0.5,
                    ),
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

    // Outline the center texel, to indicate which texel we're printing the values of:
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
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            tensor_pixel_value_ui(ui, tensor, annotations, [x as _, y as _], meaning, meter);

            // Show a big sample of the color of the middle texel:
            let (rect, _) =
                ui.allocate_exact_size(Vec2::splat(ui.available_height()), egui::Sense::hover());
            // Position texture so that the center texel is at the center of the rect:
            let zoom = rect.width();
            let image_rect_on_screen = egui::Rect::from_min_size(
                rect.center()
                    - zoom
                        * egui::vec2(
                            (center_texel[0] as f32) + 0.5,
                            (center_texel[1] as f32) + 0.5,
                        ),
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
    tensor: &TensorData,
    annotations: &Annotations,
    [x, y]: [u64; 2],
    meaning: TensorDataMeaning,
    meter: Option<f32>,
) {
    egui::Grid::new("hovered pixel properties").show(ui, |ui| {
        ui.label("Position:");
        ui.label(format!("{x}, {y}"));
        ui.end_row();

        // Check for annotations on any single-channel image
        if let Some([_, _, 1]) = tensor.image_height_width_channels() {
            if let Some(raw_value) = tensor.get(&[y, x]) {
                if let (TensorDataMeaning::ClassId, Some(u16_val)) =
                    (meaning, raw_value.try_as_u16())
                {
                    ui.label("Label:");
                    ui.label(
                        annotations
                            .resolved_class_description(Some(ClassId::from(u16_val)))
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
                let meters = raw_value / (meter as f64);
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
                if let Some([r, g, b]) = match &tensor.buffer {
                    TensorBuffer::Nv12(_) => tensor
                        .get_nv12_pixel(x, y)
                        .map(|rgb| rgb.map(TensorElement::U8)),
                    TensorBuffer::Yuy2(_) => tensor
                        .get_yuy2_pixel(x, y)
                        .map(|rgb| rgb.map(TensorElement::U8)),
                    _ => {
                        if let [Some(r), Some(g), Some(b)] = [
                            tensor.get_with_image_coords(x, y, 0),
                            tensor.get_with_image_coords(x, y, 1),
                            tensor.get_with_image_coords(x, y, 2),
                        ] {
                            Some([r, g, b])
                        } else {
                            None
                        }
                    }
                } {
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
    re_tracing::profile_function!();

    let mut histograms = [[0_u64; 256]; 3];
    {
        // TODO(emilk): this is slow, so cache the results!
        re_tracing::profile_scope!("build");
        for pixel in rgb.chunks_exact(3) {
            for c in 0..3 {
                histograms[c][pixel[c] as usize] += 1;
            }
        }
    }

    use egui_plot::{Bar, BarChart, Legend, Plot};

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

    re_tracing::profile_scope!("show");
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
fn copy_and_save_image_ui(ui: &mut egui::Ui, tensor: &TensorData, _encoded_tensor: &TensorData) {
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
                    save_image(&dynamic_image);
                }
                Err(err) => {
                    re_log::error!("Failed to convert tensor to image: {err}");
                }
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn save_image(dynamic_image: &image::DynamicImage) {
    if let Some(path) = rfd::FileDialog::new()
        .set_file_name("image.png")
        .save_file()
    {
        match dynamic_image.save(&path) {
            Ok(()) => {
                re_log::info!("Image saved to {path:?}");
            }
            Err(err) => {
                re_log::error!("Failed saving image to {path:?}: {err}");
            }
        }
    }
}
