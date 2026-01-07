use re_sdk_types::{
    components::TensorData,
    tensor_data::{TensorDataType, TensorElement},
};
use re_ui::UiExt as _;
use re_viewer_context::{Annotations, ViewContext};

use crate::{
    dimension_mapping::TensorSliceSelection, view_class::selected_tensor_slice,
    visualizer_system::TensorVisualization,
};

const ZOOMED_IMAGE_TEXEL_RADIUS: i64 = 10;
const POINTS_PER_TEXEL: f32 = 5.0;

pub fn show_tensor_hover_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    tensors: &[TensorVisualization],
    slice_selection: &TensorSliceSelection,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    _mag_filter: re_sdk_types::components::MagnificationFilter,
) {
    let Some(first_tensor) = tensors.first() else {
        return;
    };

    let Some((height, width)) =
        crate::view_class::tensor_slice_shape(&first_tensor.tensor, slice_selection)
    else {
        return;
    };

    let x_float = (pointer_pos.x - image_rect.min.x) / image_rect.width() * (width as f32);
    let y_float = (pointer_pos.y - image_rect.min.y) / image_rect.height() * (height as f32);

    let x = x_float.floor() as isize;
    let y = y_float.floor() as isize;

    if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
        return;
    }

    let x = x as usize;
    let y = y as usize;

    let mag_filter = re_sdk_types::components::MagnificationFilter::Nearest;
    let (textured_rects, texture_filter_magnification) =
        match crate::view_class::create_textured_rects_for_batch(
            ctx,
            tensors,
            slice_selection,
            mag_filter,
        ) {
            Ok(textured_rects) => textured_rects,
            Err(err) => {
                ui.error_with_details_on_hover(err.to_string());
                return;
            }
        };

    let zoom_result = ui
        .horizontal(|ui| {
            let zoom_result = show_zoomed_image_region(
                ui,
                ctx,
                &textured_rects,
                (x, y),
                texture_filter_magnification,
            );

            ui.separator();

            ui.vertical(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                tensor_hover_value_ui(ctx, ui, tensors, slice_selection, x, y);
                if let Err(err) = show_single_pixel_sample(
                    ui,
                    ctx,
                    &textured_rects,
                    (x, y),
                    texture_filter_magnification,
                ) {
                    ui.error_with_details_on_hover(err.to_string());
                }
            });

            zoom_result
        })
        .inner;

    if let Err(err) = zoom_result {
        ui.error_with_details_on_hover(err.to_string());
    }
}

#[allow(clippy::too_many_arguments)]
fn show_zoomed_image_region(
    ui: &mut egui::Ui,
    ctx: &ViewContext<'_>,
    textured_rects: &[re_renderer::renderer::TexturedRect],
    (center_x, center_y): (usize, usize),
    texture_filter_magnification: re_renderer::renderer::TextureFilterMag,
) -> anyhow::Result<()> {
    use crate::view_class::render_tensor_slice_batch;

    let zoom_rect_size =
        egui::Vec2::splat(((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32) * POINTS_PER_TEXEL);
    let (_id, zoom_rect) = ui.allocate_space(zoom_rect_size);

    let painter = ui.painter();
    painter.rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    let space_rect = egui::Rect::from_min_max(
        egui::pos2(
            center_x as f32 - ZOOMED_IMAGE_TEXEL_RADIUS as f32,
            center_y as f32 - ZOOMED_IMAGE_TEXEL_RADIUS as f32,
        ),
        egui::pos2(
            center_x as f32 + ZOOMED_IMAGE_TEXEL_RADIUS as f32 + 1.0,
            center_y as f32 + ZOOMED_IMAGE_TEXEL_RADIUS as f32 + 1.0,
        ),
    );

    render_tensor_slice_batch(
        ctx,
        &painter.with_clip_rect(zoom_rect),
        textured_rects,
        zoom_rect,
        space_rect,
        texture_filter_magnification,
    )?;

    let center_texel_rect =
        egui::Rect::from_center_size(zoom_rect.center(), egui::Vec2::splat(POINTS_PER_TEXEL));
    painter.rect_stroke(
        center_texel_rect.expand(1.0),
        0.0,
        (1.0, egui::Color32::BLACK),
        egui::StrokeKind::Outside,
    );
    painter.rect_stroke(
        center_texel_rect,
        0.0,
        (1.0, egui::Color32::WHITE),
        egui::StrokeKind::Outside,
    );

    Ok(())
}

fn show_single_pixel_sample(
    ui: &mut egui::Ui,
    ctx: &ViewContext<'_>,
    textured_rects: &[re_renderer::renderer::TexturedRect],
    (center_x, center_y): (usize, usize),
    texture_filter_magnification: re_renderer::renderer::TextureFilterMag,
) -> anyhow::Result<()> {
    use crate::view_class::render_tensor_slice_batch;

    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::splat(ui.available_height()),
        egui::Sense::hover(),
    );
    let space_rect = egui::Rect::from_min_max(
        egui::pos2(center_x as f32, center_y as f32),
        egui::pos2(center_x as f32 + 1.0, center_y as f32 + 1.0),
    );

    render_tensor_slice_batch(
        ctx,
        &ui.painter().with_clip_rect(rect),
        textured_rects,
        rect,
        space_rect,
        texture_filter_magnification,
    )?;

    Ok(())
}

fn get_tensor_value_text(
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
    x: usize,
    y: usize,
) -> Option<(String, String)> {
    macro_rules! get_text {
        ($T:ty, $variant:ident) => {{
            let view = ndarray::ArrayViewD::<$T>::try_from(&tensor.0).ok()?;
            let slice = selected_tensor_slice(slice_selection, &view);

            if slice.ndim() == 2 {
                let slice = slice.into_dimensionality::<ndarray::Ix2>().ok()?;
                if x >= slice.shape()[1] || y >= slice.shape()[0] {
                    return None;
                }
                let value = slice[[y, x]];
                Some((
                    "Val:".to_owned(),
                    format_tensor_element(TensorElement::$variant(value)),
                ))
            } else if slice.ndim() == 3 {
                let slice = slice.into_dimensionality::<ndarray::Ix3>().ok()?;
                if x >= slice.shape()[1] || y >= slice.shape()[0] {
                    return None;
                }

                let num_channels = slice.shape()[2];
                if num_channels == 1 {
                    let value = slice[[y, x, 0]];
                    Some((
                        "Val:".to_owned(),
                        format_tensor_element(TensorElement::$variant(value)),
                    ))
                } else if num_channels == 3 || num_channels == 4 {
                    let mut elements = Vec::new();
                    for c in 0..num_channels {
                        elements.push(TensorElement::$variant(slice[[y, x, c]]));
                    }
                    Some(format_pixel_value(elements))
                } else {
                    None
                }
            } else {
                None
            }
        }};
    }

    match tensor.dtype() {
        TensorDataType::U8 => get_text!(u8, U8),
        TensorDataType::U16 => get_text!(u16, U16),
        TensorDataType::U32 => get_text!(u32, U32),
        TensorDataType::U64 => get_text!(u64, U64),
        TensorDataType::I8 => get_text!(i8, I8),
        TensorDataType::I16 => get_text!(i16, I16),
        TensorDataType::I32 => get_text!(i32, I32),
        TensorDataType::I64 => get_text!(i64, I64),
        TensorDataType::F16 => get_text!(half::f16, F16),
        TensorDataType::F32 => get_text!(f32, F32),
        TensorDataType::F64 => get_text!(f64, F64),
    }
}

fn format_tensor_element(el: TensorElement) -> String {
    el.format_padded()
}

fn format_pixel_value(elements: Vec<TensorElement>) -> (String, String) {
    let values_str = elements
        .iter()
        .map(|e| e.format_padded())
        .collect::<Vec<_>>()
        .join(", ");

    if elements.len() == 3 {
        ("RGB:".to_owned(), values_str)
    } else if elements.len() == 4 {
        ("RGBA:".to_owned(), values_str)
    } else {
        ("Val:".to_owned(), values_str)
    }
}

fn tensor_hover_value_ui(
    _ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    tensors: &[TensorVisualization],
    slice_selection: &TensorSliceSelection,
    x: usize,
    y: usize,
) {
    egui::Grid::new("tensor_hover_ui")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Position:");
            ui.monospace(format!("{x}, {y}"));
            ui.end_row();

            for tensor_view in tensors {
                let TensorVisualization {
                    entity_path,
                    tensor,
                    annotations,
                    ..
                } = tensor_view;

                ui.separator();
                ui.end_row();

                ui.label("");
                ui.scope(|ui| {
                    let small = ui
                        .style()
                        .text_styles
                        .get(&egui::TextStyle::Small)
                        .cloned()
                        .unwrap_or_else(|| egui::FontId::proportional(10.0));
                    ui.style_mut().text_styles.insert(egui::TextStyle::Body, small);
                    let display_name = entity_path
                        .last()
                        .map(|part| part.ui_string())
                        .unwrap_or_else(|| "/".to_owned());
                    ui.label(display_name)
                        .on_hover_text(entity_path.to_string());
                });
                ui.end_row();

                if let Some((label, value_text)) =
                    get_tensor_value_text(tensor, slice_selection, x, y)
                {
                    ui.label(label);
                    ui.monospace(value_text);
                    ui.end_row();
                }

                if let Some(value) = get_tensor_value_at(tensor, slice_selection, x, y) {
                    if let Some(label) = get_annotation_label(value, annotations) {
                        ui.label("Label:");
                        ui.label(label);
                        ui.end_row();
                    }
                }
            }
        });
}

fn get_tensor_value_at(
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
    x: usize,
    y: usize,
) -> Option<TensorElement> {
    macro_rules! get_value {
        ($T:ty, $variant:ident) => {{
            let view = ndarray::ArrayViewD::<$T>::try_from(&tensor.0).ok()?;
            let slice = selected_tensor_slice(slice_selection, &view);
            if slice.ndim() == 2 {
                let slice = slice.into_dimensionality::<ndarray::Ix2>().ok()?;
                if x >= slice.shape()[1] || y >= slice.shape()[0] {
                    return None;
                }
                Some(TensorElement::$variant(slice[[y, x]]))
            } else if slice.ndim() == 3 {
                let slice = slice.into_dimensionality::<ndarray::Ix3>().ok()?;
                if x >= slice.shape()[1] || y >= slice.shape()[0] {
                    return None;
                }
                if slice.shape()[2] != 1 {
                    return None;
                }
                Some(TensorElement::$variant(slice[[y, x, 0]]))
            } else {
                None
            }
        }};
    }

    match tensor.dtype() {
        TensorDataType::U8 => get_value!(u8, U8),
        TensorDataType::U16 => get_value!(u16, U16),
        TensorDataType::U32 => get_value!(u32, U32),
        TensorDataType::U64 => get_value!(u64, U64),
        TensorDataType::I8 => get_value!(i8, I8),
        TensorDataType::I16 => get_value!(i16, I16),
        TensorDataType::I32 => get_value!(i32, I32),
        TensorDataType::I64 => get_value!(i64, I64),
        TensorDataType::F16 => get_value!(half::f16, F16),
        TensorDataType::F32 => get_value!(f32, F32),
        TensorDataType::F64 => get_value!(f64, F64),
    }
}

fn get_annotation_label(value: TensorElement, annotations: &Annotations) -> Option<String> {
    let class_id = match value {
        TensorElement::U8(v) => Some(v as u16),
        TensorElement::U16(v) => Some(v),
        TensorElement::U32(v) => u16::try_from(v).ok(),
        TensorElement::U64(v) => u16::try_from(v).ok(),
        TensorElement::I8(v) if v >= 0 => Some(v as u16),
        TensorElement::I16(v) if v >= 0 => Some(v as u16),
        TensorElement::I32(v) => u16::try_from(v).ok(),
        TensorElement::I64(v) => u16::try_from(v).ok(),
        _ => None,
    }?;

    let desc = annotations.resolved_class_description(Some(re_sdk_types::components::ClassId::from(class_id)));
    desc.annotation_info().label(None)
}
