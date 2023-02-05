use std::{collections::BTreeMap, fmt::Display};

use eframe::emath::Align2;
use egui::{epaint::TextShape, Color32, ColorImage, NumExt as _, Vec2};
use half::f16;
use ndarray::{Axis, Ix2};

use re_log_types::{component_types, ClassicTensor, TensorDataType};
use re_tensor_ops::dimension_mapping::{DimensionMapping, DimensionSelector};

use crate::ui::data_ui::image::tensor_dtype_and_shape_ui_grid_contents;

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

    /// Last viewed tensor, copied each frame.
    /// Used for the selection view.
    #[serde(skip)]
    tensor: Option<ClassicTensor>,
}

impl ViewTensorState {
    pub fn create(tensor: &ClassicTensor) -> ViewTensorState {
        Self {
            selector_values: Default::default(),
            dimension_mapping: DimensionMapping::create(tensor.shape()),
            color_mapping: ColorMapping::default(),
            texture_settings: TextureSettings::default(),
            tensor: Some(tensor.clone()),
        }
    }

    pub(crate) fn ui(&mut self, ctx: &mut crate::misc::ViewerContext<'_>, ui: &mut egui::Ui) {
        let Some(tensor) = &self.tensor else {
            ui.label("No Tensor shown in this Space View.");
            return;
        };

        ctx.re_ui
            .selection_grid(ui, "tensor_selection_ui")
            .show(ui, |ui| {
                tensor_dtype_and_shape_ui_grid_contents(
                    ctx.re_ui,
                    ui,
                    tensor,
                    Some(ctx.cache.tensor_stats(tensor)),
                );
                self.texture_settings.ui(ctx.re_ui, ui);
                self.color_mapping.ui(ctx.re_ui, ui);
            });

        ui.separator();
        ui.strong("Dimension Mapping");
        dimension_mapping_ui(ctx.re_ui, ui, &mut self.dimension_mapping, tensor.shape());
        let default_mapping = DimensionMapping::create(tensor.shape());
        if ui
            .add_enabled(
                self.dimension_mapping != default_mapping,
                egui::Button::new("Reset mapping"),
            )
            .on_disabled_hover_text("The default is already set up.")
            .on_hover_text("Reset dimension mapping to the default.")
            .clicked()
        {
            self.dimension_mapping = DimensionMapping::create(tensor.shape());
        }
    }
}

pub(crate) fn view_tensor(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTensorState,
    tensor: &ClassicTensor,
) {
    crate::profile_function!();

    state.tensor = Some(tensor.clone());

    if !state.dimension_mapping.is_valid(tensor.num_dim()) {
        state.dimension_mapping = DimensionMapping::create(tensor.shape());
    }

    let default_item_spacing = ui.spacing_mut().item_spacing;
    ui.spacing_mut().item_spacing.y = 0.0; // No extra spacing between sliders and tensor

    if state
        .dimension_mapping
        .selectors
        .iter()
        .any(|selector| selector.visible)
    {
        egui::Frame {
            inner_margin: egui::Margin::symmetric(16.0, 8.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = default_item_spacing; // keep the default spacing between sliders
            selectors_ui(ui, state, tensor);
        });
    }

    tensor_ui(ctx, ui, state, tensor);
}

fn tensor_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTensorState,
    tensor: &ClassicTensor,
) {
    let tensor_shape = tensor.shape();

    let tensor_stats = ctx.cache.tensor_stats(tensor);
    let range = tensor_stats.range;
    let color_mapping = &state.color_mapping;

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: u8| {
                    // We always use the full range for u8
                    color_mapping.color_from_normalized(value as f32 / 255.0)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::U16 => match re_tensor_ops::as_ndarray::<u16>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: u16| {
                    let (tensor_min, tensor_max) = range.unwrap_or((0.0, u16::MAX as f64)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::U32 => match re_tensor_ops::as_ndarray::<u32>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = range.unwrap_or((0.0, u32::MAX as f64)); // the cache should provide the range

                let color_from_value = |value: u32| {
                    color_mapping.color_from_normalized(egui::remap(
                        value as f64,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ) as f32)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::U64 => match re_tensor_ops::as_ndarray::<u64>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: u64| {
                    let (tensor_min, tensor_max) = range.unwrap_or((0.0, u64::MAX as f64)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value as f64,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ) as f32)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::I8 => match re_tensor_ops::as_ndarray::<i8>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: i8| {
                    // We always use the full range for i8:
                    let (tensor_min, tensor_max) = (i8::MIN as f32, i8::MAX as f32);
                    color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::I16 => match re_tensor_ops::as_ndarray::<i16>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: i16| {
                    let (tensor_min, tensor_max) =
                        range.unwrap_or((i16::MIN as f64, i16::MAX as f64)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::I32 => match re_tensor_ops::as_ndarray::<i32>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: i32| {
                    let (tensor_min, tensor_max) =
                        range.unwrap_or((i32::MIN as f64, i32::MAX as f64)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value as f64,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ) as f32)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::I64 => match re_tensor_ops::as_ndarray::<i64>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: i64| {
                    let (tensor_min, tensor_max) =
                        range.unwrap_or((i64::MIN as f64, i64::MAX as f64)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value as f64,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ) as f32)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::F16 => match re_tensor_ops::as_ndarray::<f16>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: f16| {
                    let (tensor_min, tensor_max) = range.unwrap_or((0.0, 1.0)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value.to_f32(),
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::F32 => match re_tensor_ops::as_ndarray::<f32>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: f32| {
                    let (tensor_min, tensor_max) = range.unwrap_or((0.0, 1.0)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },

        TensorDataType::F64 => match re_tensor_ops::as_ndarray::<f64>(tensor) {
            Ok(tensor) => {
                let color_from_value = |value: f64| {
                    let (tensor_min, tensor_max) = range.unwrap_or((0.0, 1.0)); // the cache should provide the range
                    color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ) as f32)
                };

                let slice = selected_tensor_slice(state, &tensor);
                slice_ui(ctx, ui, state, tensor_shape, slice, color_from_value);
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(err.to_string()));
            }
        },
    }
}

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum ColorMap {
    Greyscale,
    Turbo,
    Virdis,
}

impl std::fmt::Display for ColorMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ColorMap::Greyscale => "Greyscale",
            ColorMap::Turbo => "Turbo",
            ColorMap::Virdis => "Viridis",
        })
    }
}

/// How we map values to colors.
#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ColorMapping {
    map: ColorMap,
    gamma: f32,
}

impl Default for ColorMapping {
    fn default() -> Self {
        Self {
            map: ColorMap::Virdis,
            gamma: 1.0,
        }
    }
}

impl ColorMapping {
    pub fn color_from_normalized(&self, f: f32) -> Color32 {
        let f = f.powf(self.gamma);

        match self.map {
            ColorMap::Greyscale => {
                let lum = (f * 255.0 + 0.5) as u8;
                Color32::from_gray(lum)
            }
            ColorMap::Turbo => crate::misc::color_map::turbo_color_map(f),
            ColorMap::Virdis => {
                let [r, g, b] = crate::misc::color_map::viridis_color_map(f);
                Color32::from_rgb(r, g, b)
            }
        }
    }

    fn ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let ColorMapping { map, gamma } = self;

        re_ui.grid_left_hand_label(ui, "Color map");
        egui::ComboBox::from_id_source("color map select")
            .selected_text(map.to_string())
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.selectable_value(map, ColorMap::Greyscale, ColorMap::Greyscale.to_string());
                ui.selectable_value(map, ColorMap::Virdis, ColorMap::Virdis.to_string());
                ui.selectable_value(map, ColorMap::Turbo, ColorMap::Turbo.to_string());
            });
        ui.end_row();

        re_ui.grid_left_hand_label(ui, "Brightness");
        let mut brightness = 1.0 / *gamma;
        ui.add(egui::Slider::new(&mut brightness, 0.1..=10.0).logarithmic(true));
        *gamma = 1.0 / brightness;
        ui.end_row();
    }
}

// ----------------------------------------------------------------------------

/// Should we scale the rendered texture, and if so, how?
#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
enum TextureScaling {
    /// No scaling, texture size will match the tensor's width/height dimensions.
    Original,

    /// Scale the texture for the largest possible fit in the UI container.
    Fill,
}

impl Default for TextureScaling {
    fn default() -> Self {
        Self::Fill
    }
}

impl Display for TextureScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureScaling::Original => "Original".fmt(f),
            TextureScaling::Fill => "Fill".fmt(f),
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
    ) -> (egui::Response, egui::Painter, egui::Rect) {
        let img_size = egui::vec2(image.size[0] as _, image.size[1] as _);
        let img_size = Vec2::max(Vec2::splat(1.0), img_size); // better safe than sorry
        let desired_size = match self.scaling {
            TextureScaling::Original => img_size + margin,
            TextureScaling::Fill => {
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

        (response, painter, image_rect)
    }
}

// ui
impl TextureSettings {
    fn ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let TextureSettings {
            keep_aspect_ratio,
            scaling,
            options,
        } = self;

        re_ui.grid_left_hand_label(ui, "Scale");
        ui.vertical(|ui| {
            egui::ComboBox::from_id_source("texture_scaling")
                .selected_text(scaling.to_string())
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    let mut selectable_value =
                        |ui: &mut egui::Ui, e| ui.selectable_value(scaling, e, e.to_string());
                    selectable_value(ui, TextureScaling::Original);
                    selectable_value(ui, TextureScaling::Fill);
                });
            if *scaling == TextureScaling::Fill {
                ui.checkbox(keep_aspect_ratio, "Keep aspect ratio");
            }
        });
        ui.end_row();

        re_ui.grid_left_hand_label(ui, "Filtering");
        fn tf_to_string(tf: egui::TextureFilter) -> &'static str {
            match tf {
                egui::TextureFilter::Nearest => "Nearest",
                egui::TextureFilter::Linear => "Linear",
            }
        }
        egui::ComboBox::from_id_source("texture_filter")
            .selected_text(tf_to_string(options.magnification))
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(64.0);

                let mut selectable_value = |ui: &mut egui::Ui, e| {
                    ui.selectable_value(&mut options.magnification, e, tf_to_string(e))
                };
                selectable_value(ui, egui::TextureFilter::Linear);
                selectable_value(ui, egui::TextureFilter::Nearest);
            });
        ui.end_row();
    }
}

// ----------------------------------------------------------------------------

fn selected_tensor_slice<'a, T: Copy>(
    state: &ViewTensorState,
    tensor: &'a ndarray::ArrayViewD<'_, T>,
) -> ndarray::ArrayViewD<'a, T> {
    let dim_mapping = &state.dimension_mapping;

    assert!(dim_mapping.is_valid(tensor.ndim()));

    // TODO(andreas) - shouldn't just give up here
    if dim_mapping.width.is_none() || dim_mapping.height.is_none() {
        return tensor.view();
    }

    let axis = dim_mapping
        .height
        .into_iter()
        .chain(dim_mapping.width.into_iter())
        .chain(dim_mapping.selectors.iter().map(|s| s.dim_idx))
        .collect::<Vec<_>>();
    let mut slice = tensor.view().permuted_axes(axis);

    for DimensionSelector { dim_idx, .. } in &dim_mapping.selectors {
        let selector_value = state
            .selector_values
            .get(dim_idx)
            .copied()
            .unwrap_or_default() as usize;
        assert!(
            selector_value < slice.shape()[2],
            "Bad tensor slicing. Trying to select slice index {selector_value} of dim=2. tensor shape: {:?}, dim_mapping: {dim_mapping:#?}",
            tensor.shape()
        );

        // 0 and 1 are width/height, the rest are rearranged by dimension_mapping.selectors
        // This call removes Axis(2), so the next iteration of the loop does the right thing again.
        slice.index_axis_inplace(Axis(2), selector_value);
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
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_state: &ViewTensorState,
    tensor_shape: &[component_types::TensorDimension],
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
        egui::ScrollArea::both().show(ui, |ui| {
            image_ui(ui, view_state, image, dimension_labels);
        });
    } else {
        ui.label(ctx.re_ui.error_text(format!(
            "Only 2D slices supported at the moment, but slice ndim {ndims}"
        )));
    }
}

fn dimension_name(shape: &[component_types::TensorDimension], dim_idx: usize) -> String {
    let dim = &shape[dim_idx];
    dim.name.as_ref().map_or_else(
        || format!("Dimension {dim_idx} (size={})", dim.size),
        |name| format!("{name} (size={})", dim.size),
    )
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

    let font_id = egui::TextStyle::Body.resolve(ui.style());

    let margin = egui::vec2(0.0, 0.0);

    let (response, painter, image_rect) =
        view_state.texture_settings.paint_image(ui, margin, image);

    if !response.hovered() {
        paint_axis_names(ui, &painter, image_rect, font_id, dimension_labels);
    }
}

fn paint_axis_names(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    font_id: egui::FontId,
    dimension_labels: [(String, bool); 2],
) {
    // Show axis names etc:
    let [(width_name, invert_width), (height_name, invert_height)] = dimension_labels;
    let text_color = ui.visuals().text_color();

    let rounding = re_ui::ReUi::normal_rounding();
    let inner_margin = rounding;
    let outer_margin = 8.0;

    let rect = rect.shrink(outer_margin + inner_margin);

    // We make sure that the label for the X axis is always at Y=0,
    // and that the label for the Y axis is always at X=0, no matter what inversions.
    //
    // For instance, with origin in the top right:
    //
    // foo ⬅
    // ..........
    // ..........
    // ..........
    // .......... ↓
    // .......... b
    // .......... a
    // .......... r

    // TODO(emilk): draw actual arrows behind the text instead of the ugly emoji arrows

    let paint_text_bg = |text_background, text_rect: egui::Rect| {
        painter.set(
            text_background,
            egui::Shape::rect_filled(
                text_rect.expand(inner_margin),
                rounding,
                ui.visuals().panel_fill,
            ),
        );
    };

    // Label for X axis:
    {
        let text_background = painter.add(egui::Shape::Noop);
        let text_rect = if invert_width {
            // On left, pointing left:
            let (pos, align) = if invert_height {
                (rect.left_bottom(), Align2::LEFT_BOTTOM)
            } else {
                (rect.left_top(), Align2::LEFT_TOP)
            };
            painter.text(
                pos,
                align,
                format!("{width_name} ⬅"),
                font_id.clone(),
                text_color,
            )
        } else {
            // On right, pointing right:
            let (pos, align) = if invert_height {
                (rect.right_bottom(), Align2::RIGHT_BOTTOM)
            } else {
                (rect.right_top(), Align2::RIGHT_TOP)
            };
            painter.text(
                pos,
                align,
                format!("➡ {width_name}"),
                font_id.clone(),
                text_color,
            )
        };
        paint_text_bg(text_background, text_rect);
    }

    // Label for Y axis:
    {
        let text_background = painter.add(egui::Shape::Noop);
        let text_rect = if invert_height {
            // On top, pointing up:
            let galley = painter.layout_no_wrap(format!("➡ {height_name}"), font_id, text_color);
            let galley_size = galley.size();
            let pos = if invert_width {
                rect.right_top() + egui::vec2(-galley_size.y, galley_size.x)
            } else {
                rect.left_top() + egui::vec2(0.0, galley_size.x)
            };
            painter.add(TextShape {
                pos,
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
            egui::Rect::from_min_size(
                pos - galley_size.x * egui::Vec2::Y,
                egui::vec2(galley_size.y, galley_size.x),
            )
        } else {
            // On bottom, pointing down:
            let galley = painter.layout_no_wrap(format!("{height_name} ⬅"), font_id, text_color);
            let galley_size = galley.size();
            let pos = if invert_width {
                rect.right_bottom() - egui::vec2(galley_size.y, 0.0)
            } else {
                rect.left_bottom()
            };
            painter.add(TextShape {
                pos,
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
            egui::Rect::from_min_size(
                pos - galley_size.x * egui::Vec2::Y,
                egui::vec2(galley_size.y, galley_size.x),
            )
        };
        paint_text_bg(text_background, text_rect);
    }
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut ViewTensorState, tensor: &ClassicTensor) {
    for selector in &state.dimension_mapping.selectors {
        if !selector.visible {
            continue;
        }

        let dim = &tensor.shape()[selector.dim_idx];
        let size = dim.size;

        let selector_value = state
            .selector_values
            .entry(selector.dim_idx)
            .or_insert_with(|| size / 2); // start in the middle

        if size > 0 {
            *selector_value = selector_value.at_most(size - 1);
        }

        if size > 1 {
            ui.horizontal(|ui| {
                let name = dim
                    .name
                    .clone()
                    .unwrap_or_else(|| selector.dim_idx.to_string());

                let slider_tooltip = format!("Adjust the selected slice for the {name} dimension");
                ui.label(&name).on_hover_text(&slider_tooltip);

                // If the range is big (say, 2048) then we would need
                // a slider that is 2048 pixels wide to get the good precision.
                // So we add a high-precision drag-value instead:
                ui.add(
                    egui::DragValue::new(selector_value)
                        .clamp_range(0..=size - 1)
                        .speed(0.5),
                )
                .on_hover_text(format!(
                    "Drag to precisely control the slice index of the {name} dimension"
                ));

                // Make the slider as big as needed:
                const MIN_SLIDER_WIDTH: f32 = 64.0;
                if ui.available_width() >= MIN_SLIDER_WIDTH {
                    ui.spacing_mut().slider_width = (size as f32 * 4.0)
                        .at_least(MIN_SLIDER_WIDTH)
                        .at_most(ui.available_width());
                    ui.add(egui::Slider::new(selector_value, 0..=size - 1).show_value(false))
                        .on_hover_text(slider_tooltip);
                }
            });
        }
    }
}
