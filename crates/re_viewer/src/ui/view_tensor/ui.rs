use std::{collections::BTreeMap, fmt::Display};

use eframe::emath::Align2;
use egui::{epaint::TextShape, Color32, ColorImage, Vec2};
use ndarray::{Axis, Ix2};
use re_log_types::{Tensor, TensorDataMeaning, TensorDataType, TensorDimension};
use re_tensor_ops::dimension_mapping::DimensionMapping;

use super::dimension_mapping_ui;

// ---

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ViewTensorState {
    /// How we select which dimensions to project the tensor onto.
    dimension_mapping: DimensionMapping,

    /// Selected value of every dimension (iff they are in [`DimensionMapping::selectors`]).
    selector_values: BTreeMap<usize, u64>,

    /// How we map values to colors.
    color_mapping: ColorMapping,

    /// Scaling, filtering, aspect ratio, etc for the rendered texture.
    texture_settings: TextureSettings,

    // last viewed tensor, copied each frame
    #[serde(skip)]
    #[serde(default = "empty_tensor")]
    tensor: Tensor,
}

fn empty_tensor() -> Tensor {
    Tensor {
        tensor_id: re_log_types::TensorId(uuid::uuid!("7c8c3d2b-30f1-4206-844d-c43790912492")),
        shape: vec![TensorDimension::unnamed(0)],
        dtype: TensorDataType::U8,
        meaning: TensorDataMeaning::Unknown,
        data: re_log_types::TensorDataStore::Dense(vec![].into()),
    }
}

impl ViewTensorState {
    pub fn create(tensor: &Tensor) -> ViewTensorState {
        Self {
            selector_values: Default::default(),
            dimension_mapping: DimensionMapping::create(tensor.num_dim()),
            color_mapping: ColorMapping::default(),
            texture_settings: TextureSettings::default(),
            tensor: tensor.clone(),
        }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Dimension Mapping", |ui| {
            ui.label(format!("shape: {:?}", self.tensor.shape));
            ui.label(format!("dtype: {:?}", self.tensor.dtype));
            ui.add_space(12.0);

            dimension_mapping_ui(ui, &mut self.dimension_mapping, &self.tensor.shape);
        });

        self.texture_settings.show(ui);

        color_mapping_ui(ui, &mut self.color_mapping);
    }
}

pub(crate) fn view_tensor(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTensorState,
    tensor: &Tensor,
) {
    crate::profile_function!();

    state.tensor = tensor.clone();

    selectors_ui(ui, state, tensor);

    let tensor_shape = &tensor.shape;

    let tensor_stats = ctx.cache.tensor_stats(tensor);
    let range = tensor_stats.range;

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: u8| {
                    state
                        .color_mapping
                        .color_from_normalized(value as f32 / 255.0)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::U16 => match re_tensor_ops::as_ndarray::<u16>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = range.unwrap_or((0.0, u16::MAX as f64)); // the cache should provide the range
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));

                let color_from_value = |value: u16| {
                    state.color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::F32 => match re_tensor_ops::as_ndarray::<f32>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = range.unwrap_or((0.0, 1.0)); // the cache should provide the range
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));

                let color_from_value = |value: f32| {
                    state.color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },
    }
}

// ----------------------------------------------------------------------------

/// How we map values to colors.
#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ColorMapping {
    turbo: bool,
    gamma: f32,
}

impl Default for ColorMapping {
    fn default() -> Self {
        Self {
            turbo: true,
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
    ui.group(|ui| {
        ui.strong("Color map");

        ui.horizontal(|ui| {
            ui.radio_value(&mut color_mapping.turbo, false, "Grayscale");
            ui.radio_value(&mut color_mapping.turbo, true, "Turbo");
        });

        let mut brightness = 1.0 / color_mapping.gamma;
        ui.add(
            egui::Slider::new(&mut brightness, 0.1..=10.0)
                .logarithmic(true)
                .text("Brightness"),
        );
        color_mapping.gamma = 1.0 / brightness;
    });
}

// ----------------------------------------------------------------------------

/// Should we scale the rendered texture, and if so, how?
#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
enum TextureScaling {
    /// No scaling, texture size will match the tensor's width/height dimensions.
    None,
    /// Scale the texture for the largest possible fit in the UI container.
    Fit,
}

impl Default for TextureScaling {
    fn default() -> Self {
        Self::Fit
    }
}

impl Display for TextureScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureScaling::None => "None".fmt(f),
            TextureScaling::Fit => "Fit".fmt(f),
        }
    }
}

/// Scaling, filtering, aspect ratio, etc for the rendered texture.
#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct TextureSettings {
    /// Should the aspect ratio of the tensor be kept when scaling?
    keep_aspect_ratio: bool,

    /// Should we scale the texture when rendering?
    scaling: TextureScaling,

    /// Specifies the sampling filter used to render the texture.
    options: egui::TextureOptions,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            keep_aspect_ratio: true,
            scaling: TextureScaling::default(),
            options: egui::TextureOptions {
                // This is best for low-res depth-images and the like
                magnification: egui::TextureFilter::Nearest,
                minification: egui::TextureFilter::Linear,
            },
        }
    }
}

// helpers
impl TextureSettings {
    fn paint_image(
        &self,
        ui: &mut egui::Ui,
        margin: Vec2,
        image: ColorImage,
    ) -> (egui::Painter, egui::Rect) {
        let img_size = egui::vec2(image.size[0] as _, image.size[1] as _);
        let img_size = Vec2::max(Vec2::splat(1.0), img_size); // better safe than sorry
        let desired_size = match self.scaling {
            TextureScaling::None => img_size + margin,
            TextureScaling::Fit => {
                let desired_size = ui.available_size() - margin;
                if self.keep_aspect_ratio {
                    let scale = (desired_size / img_size).min_elem();
                    img_size * scale
                } else {
                    desired_size
                }
            }
        };

        // TODO(cmc): don't recreate texture unless necessary
        let texture = ui.ctx().load_texture("tensor_slice", image, self.options);

        let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
        let rect = response.rect;
        let image_rect = egui::Rect::from_min_max(rect.min + margin, rect.max);

        let mut mesh = egui::Mesh::with_texture(texture.id());
        let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        mesh.add_rect_with_uv(image_rect, uv, Color32::WHITE);

        painter.add(mesh);

        (painter, image_rect)
    }
}

// ui
impl TextureSettings {
    fn show(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Texture scaling")
                    .selected_text(self.scaling.to_string())
                    .show_ui(ui, |ui| {
                        let mut selectable_value = |ui: &mut egui::Ui, e| {
                            ui.selectable_value(&mut self.scaling, e, e.to_string())
                        };
                        selectable_value(ui, TextureScaling::None);
                        selectable_value(ui, TextureScaling::Fit);
                    });
                ui.checkbox(&mut self.keep_aspect_ratio, "Keep aspect ratio");
            });

            texture_filter_ui(
                ui,
                "Texture magnification filter",
                &mut self.options.magnification,
            );
        });
    }
}

fn texture_filter_ui(ui: &mut egui::Ui, label: &str, filter: &mut egui::TextureFilter) {
    fn tf_to_string(tf: egui::TextureFilter) -> &'static str {
        match tf {
            egui::TextureFilter::Nearest => "Nearest",
            egui::TextureFilter::Linear => "Linear",
        }
    }

    egui::ComboBox::from_label(label)
        .selected_text(tf_to_string(*filter))
        .show_ui(ui, |ui| {
            let mut selectable_value =
                |ui: &mut egui::Ui, e| ui.selectable_value(filter, e, tf_to_string(e));
            selectable_value(ui, egui::TextureFilter::Linear);
            selectable_value(ui, egui::TextureFilter::Nearest);
        });
}

// ----------------------------------------------------------------------------

fn selected_tensor_slice<'a, T: Copy>(
    state: &ViewTensorState,
    tensor: &'a ndarray::ArrayViewD<'_, T>,
) -> ndarray::ArrayViewD<'a, T> {
    let dim_mapping = &state.dimension_mapping;

    // TODO(andreas) - shouldn't just give up here
    if dim_mapping.width.is_none() || dim_mapping.height.is_none() {
        return tensor.view();
    }

    let axis = dim_mapping
        .height
        .into_iter()
        .chain(dim_mapping.width.into_iter())
        .chain(dim_mapping.channel.into_iter())
        .chain(dim_mapping.selectors.iter().copied())
        .collect::<Vec<_>>();
    let mut slice = tensor.view().permuted_axes(axis);

    for dim_idx in &dim_mapping.selectors {
        let selector_value = state
            .selector_values
            .get(dim_idx)
            .copied()
            .unwrap_or_default();
        // 0 and 1 are width/height, the rest are rearranged by dimension_mapping.selectors
        // This call removes Axis(2), so the next iteration of the loop does the right thing again.
        slice.index_axis_inplace(Axis(2), selector_value as _);
    }
    if dim_mapping.invert_height {
        slice.invert_axis(Axis(0));
    }
    if dim_mapping.invert_width {
        slice.invert_axis(Axis(1));
    }

    slice
}

fn slice_ui<T: Copy>(
    ui: &mut egui::Ui,
    view_state: &ViewTensorState,
    tensor_shape: &[TensorDimension],
    slice: ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) {
    crate::profile_function!();

    let ndims = slice.ndim();
    if let Ok(slice) = slice.into_dimensionality::<Ix2>() {
        let dimension_labels = {
            let dm = &view_state.dimension_mapping;
            [
                (
                    dimension_name(tensor_shape, dm.width.unwrap()),
                    dm.invert_width,
                ),
                (
                    dimension_name(tensor_shape, dm.height.unwrap()),
                    dm.invert_height,
                ),
            ]
        };

        let image = into_image(&slice, color_from_value);
        image_ui(ui, view_state, image, dimension_labels);
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

fn image_ui(
    ui: &mut egui::Ui,
    view_state: &ViewTensorState,
    image: ColorImage,
    dimension_labels: [(String, bool); 2],
) {
    crate::profile_function!();

    egui::ScrollArea::both().show(ui, |ui| {
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let margin = Vec2::splat(font_id.size + 2.0);

        let (painter, image_rect) = view_state.texture_settings.paint_image(ui, margin, image);

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
                pos: image_rect.left_top() - egui::vec2(galley.size().y, -galley.size().x),
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
        } else {
            let galley = painter.layout_no_wrap(format!("{height_name} ⬅"), font_id, text_color);
            painter.add(TextShape {
                pos: image_rect.left_bottom() - egui::vec2(galley.size().y, 0.0),
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
        }
    });
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut ViewTensorState, tensor: &Tensor) {
    if state.dimension_mapping.selectors.is_empty() {
        return;
    }

    ui.group(|ui| {
        if state.dimension_mapping.selectors.len() == 1 {
            ui.label("Slice selector:");
        } else {
            ui.label("Slice selectors:");
        }

        for &dim_idx in &state.dimension_mapping.selectors {
            let dim = &tensor.shape[dim_idx];
            if dim.size > 1 {
                let selector_value = state
                    .selector_values
                    .entry(dim_idx)
                    .or_insert_with(|| dim.size / 2); // start in the middle

                ui.add(
                    egui::Slider::new(selector_value, 0..=dim.size - 1)
                        .text(dimension_name(&tensor.shape, dim_idx)),
                );
            }
        }
    });
}
