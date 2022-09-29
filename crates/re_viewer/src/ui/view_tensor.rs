use ndarray::{Axis, Ix2};
use re_log_types::{Tensor, TensorDataType, TensorDimension};
use re_tensor_ops::dimension_mapping::DimensionMapping;

use egui::{epaint::TextShape, Color32, ColorImage};

use crate::ui::tensor_dimension_mapper::dimension_mapping_ui;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TensorViewState {
    /// How we select which dimensions to project the tensor onto.
    dimension_mapping: DimensionMapping,

    /// Selected value of every selector mapping (in order of DimensionMapping::selectors)
    /// I.e. selector_values gives you the selected index for the dimension specified by DimensionMapping::selectors
    selector_values: Vec<u64>,

    /// How we map values to colors.
    color_mapping: ColorMapping,
}

impl TensorViewState {
    pub(crate) fn create(tensor: &re_log_types::Tensor) -> TensorViewState {
        Self {
            selector_values: Default::default(),
            dimension_mapping: DimensionMapping::create(tensor),
            color_mapping: ColorMapping::default(),
        }
    }
}

/// How we map values to colors.
#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ColorMapping {
    turbo: bool,
    gamma: f32,
}

impl Default for ColorMapping {
    fn default() -> Self {
        Self {
            turbo: false,
            gamma: 1.0,
        }
    }
}

impl ColorMapping {
    pub fn color_from_normalized(&self, f: f32) -> Color32 {
        let f = f.powf(self.gamma);

        if self.turbo {
            let [r, g, b] = crate::misc::color_map::turbo_color_map(f);
            Color32::from_rgb(r, g, b)
        } else {
            let lum = (f * 255.0 + 0.5) as u8;
            Color32::from_gray(lum)
        }
    }
}

fn color_mapping_ui(ui: &mut egui::Ui, color_mapping: &mut ColorMapping) {
    ui.horizontal(|ui| {
        ui.label("Color map:");
        let mut brightness = 1.0 / color_mapping.gamma;
        ui.add(
            egui::Slider::new(&mut brightness, 0.1..=10.0)
                .logarithmic(true)
                .text("Brightness"),
        );
        color_mapping.gamma = 1.0 / brightness;
        ui.checkbox(&mut color_mapping.turbo, "Turbo colormap");
    });
}

// ----------------------------------------------------------------------------

pub(crate) fn view_tensor(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    crate::profile_function!();

    ui.collapsing("Dimension Mapping", |ui| {
        ui.label(format!("shape: {:?}", tensor.shape));
        ui.label(format!("dtype: {:?}", tensor.dtype));
        ui.add_space(12.0);

        dimension_mapping_ui(ui, &mut state.dimension_mapping, &tensor.shape);
    });

    selectors_ui(ui, state, tensor);

    let tensor_shape = &tensor.shape;

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                color_mapping_ui(ui, &mut state.color_mapping);

                let color_from_value = |value: u8| {
                    state
                        .color_mapping
                        .color_from_normalized(value as f32 / 255.0)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(
                    ui,
                    tensor_shape,
                    &state.dimension_mapping,
                    slice,
                    color_from_value,
                );
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::U16 => match re_tensor_ops::as_ndarray::<u16>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_u16(&tensor);

                ui.group(|ui| {
                    ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                    color_mapping_ui(ui, &mut state.color_mapping);
                });

                let color_from_value = |value: u16| {
                    state.color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(
                    ui,
                    tensor_shape,
                    &state.dimension_mapping,
                    slice,
                    color_from_value,
                );
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::F32 => match re_tensor_ops::as_ndarray::<f32>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_f32(&tensor);

                ui.group(|ui| {
                    ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                    color_mapping_ui(ui, &mut state.color_mapping);
                });

                let color_from_value = |value: f32| {
                    state.color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(
                    ui,
                    tensor_shape,
                    &state.dimension_mapping,
                    slice,
                    color_from_value,
                );
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },
    }
}

fn selected_tensor_slice<'a, T: Copy>(
    state: &TensorViewState,
    tensor: &'a ndarray::ArrayViewD<'_, T>,
) -> ndarray::ArrayViewD<'a, T> {
    // TODO(andreas) - shouldn't just give up here
    if state.dimension_mapping.width.is_none() || state.dimension_mapping.height.is_none() {
        return tensor.view();
    }

    let axis = [
        state.dimension_mapping.height.unwrap(),
        state.dimension_mapping.width.unwrap(),
    ]
    .into_iter()
    .chain(state.dimension_mapping.selectors.iter().copied())
    .collect::<Vec<_>>();
    let mut slice = tensor.view().permuted_axes(axis);
    for selector_value in &state.selector_values {
        // 0 and 1 are width/height, the rest are rearranged by dimension_mapping.selectors
        slice.index_axis_inplace(Axis(2), *selector_value as _);
    }
    if state.dimension_mapping.invert_height {
        slice.invert_axis(Axis(0));
    }
    if state.dimension_mapping.invert_width {
        slice.invert_axis(Axis(1));
    }

    slice
}

fn slice_ui<T: Copy>(
    ui: &mut egui::Ui,
    tensor_shape: &[TensorDimension],
    dimension_mapping: &DimensionMapping,
    slice: ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) {
    ui.monospace(format!("Slice shape: {:?}", slice.shape()));
    let ndims = slice.ndim();
    if let Ok(slice) = slice.into_dimensionality::<Ix2>() {
        let dimension_labels = [
            (
                dimension_name(tensor_shape, dimension_mapping.width.unwrap()),
                dimension_mapping.invert_width,
            ),
            (
                dimension_name(tensor_shape, dimension_mapping.height.unwrap()),
                dimension_mapping.invert_height,
            ),
        ];

        let image = into_image(&slice, color_from_value);
        image_ui(ui, image, dimension_labels);
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            format!(
                "Only 2D slices supported at the moment, but slice ndim {}",
                ndims
            ),
        );
    }
}

fn dimension_name(shape: &[TensorDimension], dim_idx: usize) -> String {
    let dim = &shape[dim_idx];
    if dim.name.is_empty() {
        format!("Dimension {} ({})", dim_idx, dim.size)
    } else {
        format!("{} ({})", dim.name, dim.size)
    }
}

fn into_image<T: Copy>(
    slice: &ndarray::ArrayView2<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) -> ColorImage {
    crate::profile_function!();

    use ndarray::Dimension as _;
    let (height, width) = slice.raw_dim().into_pattern();
    let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);

    let image_view =
        ndarray::ArrayViewMut2::from_shape(slice.raw_dim(), image.pixels.as_mut_slice())
            .expect("Mismatched length.");

    crate::profile_scope!("color_mapper");
    ndarray::Zip::from(image_view)
        .and(slice)
        .for_each(|pixel, value| {
            *pixel = color_from_value(*value);
        });

    image
}

fn image_ui(ui: &mut egui::Ui, image: ColorImage, dimension_labels: [(String, bool); 2]) {
    use egui::*;

    crate::profile_function!();
    // TODO(emilk): cache texture - don't create a new texture every frame
    let texture = ui
        .ctx()
        .load_texture("tensor_slice", image, egui::TextureFilter::Linear);
    egui::ScrollArea::both().show(ui, |ui| {
        let font_id = TextStyle::Body.resolve(ui.style());

        let margin = Vec2::splat(font_id.size + 2.0);

        let desired_size = texture.size_vec2() + margin;
        let (response, painter) = ui.allocate_painter(desired_size, Sense::hover());
        let rect = response.rect;

        let image_rect = Rect::from_min_max(rect.min + margin, rect.max);

        let mut mesh = egui::Mesh::with_texture(texture.id());
        let uv = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        mesh.add_rect_with_uv(image_rect, uv, Color32::WHITE);
        painter.add(mesh);

        let [(width_name, invert_width), (height_name, invert_height)] = dimension_labels;

        let text_color = ui.visuals().text_color();

        // Label for X axis, on top:
        if invert_width {
            painter.text(
                image_rect.left_top(),
                Align2::LEFT_BOTTOM,
                format!("{width_name} ⬅"),
                font_id.clone(),
                text_color,
            );
        } else {
            painter.text(
                image_rect.right_top(),
                Align2::RIGHT_BOTTOM,
                format!("➡ {width_name}"),
                font_id.clone(),
                text_color,
            );
        }

        // Label for Y axis, on the left:
        if invert_height {
            let galley = painter.layout_no_wrap(format!("➡ {height_name}"), font_id, text_color);
            painter.add(TextShape {
                pos: image_rect.left_top() - vec2(galley.size().y, -galley.size().x),
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
        } else {
            let galley = painter.layout_no_wrap(format!("{height_name} ⬅"), font_id, text_color);
            painter.add(TextShape {
                pos: image_rect.left_bottom() - vec2(galley.size().y, 0.0),
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
        }
    });
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    if state.dimension_mapping.selectors.is_empty() {
        return;
    }
    state
        .selector_values
        .resize(state.dimension_mapping.selectors.len(), 0);

    ui.group(|ui| {
        if state.dimension_mapping.selectors.len() == 1 {
            ui.label("Slice selector:");
        } else {
            ui.label("Slice selectors:");
        }

        for (&dim_idx, selector_value) in state
            .dimension_mapping
            .selectors
            .iter()
            .zip(state.selector_values.iter_mut())
        {
            let dim = &tensor.shape[dim_idx];
            let name = if dim.name.is_empty() {
                dim_idx.to_string()
            } else {
                dim.name.clone()
            };
            if dim.size > 1 {
                ui.add(egui::Slider::new(selector_value, 0..=dim.size - 1).text(name));
            }
        }
    });
}

fn tensor_range_f32(tensor: &ndarray::ArrayViewD<'_, f32>) -> (f32, f32) {
    crate::profile_function!();
    tensor.fold((f32::INFINITY, f32::NEG_INFINITY), |cur, &value| {
        (cur.0.min(value), cur.1.max(value))
    })
}

fn tensor_range_u16(tensor: &ndarray::ArrayViewD<'_, u16>) -> (u16, u16) {
    crate::profile_function!();
    tensor.fold((u16::MAX, u16::MIN), |cur, &value| {
        (cur.0.min(value), cur.1.max(value))
    })
}
