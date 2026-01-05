use re_data_ui::item_ui;
use re_sdk_types::components::TensorData;
use re_sdk_types::tensor_data::{TensorDataType, TensorElement};
use re_viewer_context::{Annotations, ViewerContext};

use crate::{dimension_mapping::TensorSliceSelection, view_class::selected_tensor_slice, visualizer_system::TensorVisualization};

pub fn show_tensor_hover_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    tensors: &[TensorVisualization],
    slice_selection: &TensorSliceSelection,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
) {
    let Some(first_tensor) = tensors.first() else {
        return;
    };

    let Some((height, width)) = get_tensor_slice_shape(&first_tensor.tensor, slice_selection) else {
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

                ui.label(entity_path.to_string());

                if let Some((label, value_text)) =
                    get_tensor_value_text(tensor, slice_selection, x, y)
                {
                    ui.label(label);
                    ui.monospace(value_text);
                    ui.end_row();
                }

                // If integer, check annotations, and show it in a separate row.
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

fn get_tensor_slice_shape(
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
) -> Option<(usize, usize)> {
    // The slice shape depends on the mapping.
    // The most robust way to get the shape is to create a dummy slice.
    macro_rules! get_shape {
        ($T:ty) => {{
            let view = ndarray::ArrayViewD::<$T>::try_from(&tensor.0).ok()?;
            let slice = selected_tensor_slice(slice_selection, &view);
            let shape = slice.shape();
            if shape.len() >= 2 {
                Some((shape[0], shape[1]))
            } else {
                None
            }
        }};
    }

    match tensor.dtype() {
        TensorDataType::U8 => get_shape!(u8),
        TensorDataType::U16 => get_shape!(u16),
        TensorDataType::U32 => get_shape!(u32),
        TensorDataType::U64 => get_shape!(u64),
        TensorDataType::I8 => get_shape!(i8),
        TensorDataType::I16 => get_shape!(i16),
        TensorDataType::I32 => get_shape!(i32),
        TensorDataType::I64 => get_shape!(i64),
        TensorDataType::F16 => get_shape!(half::f16),
        TensorDataType::F32 => get_shape!(f32),
        TensorDataType::F64 => get_shape!(f64),
    }
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
                if num_channels == 3 || num_channels == 4 {
                    let mut elements = Vec::new();
                    for c in 0..num_channels {
                        elements.push(TensorElement::$variant(slice[[y, x, c]]));
                    }
                    Some(format_pixel_value(elements))
                } else {
                    None // Not a color image
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
            let slice = slice.into_dimensionality::<ndarray::Ix2>().ok()?;
            if x >= slice.shape()[1] || y >= slice.shape()[0] {
                return None;
            }
            Some(TensorElement::$variant(slice[[y, x]]))
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