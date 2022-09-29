use ndarray::Ix2;
use re_log_types::{Tensor, TensorDataType};

use egui::{Color32, ColorImage};
use itertools::Itertools as _;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TensorViewState {
    /// How we select which dimensions to project the tensor onto.
    rank_mapping: RankMapping,

    /// Maps dimenion to the slice of that dimension.
    selectors: ahash::HashMap<usize, u64>,

    /// How we map values to colors.
    color_mapping: ColorMapping,
}

impl TensorViewState {
    pub(crate) fn create(tensor: &re_log_types::Tensor) -> TensorViewState {
        Self {
            selectors: Default::default(),
            rank_mapping: RankMapping::create(tensor),
            color_mapping: ColorMapping::default(),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct RankMapping {
    /// Which dimensions have selectors?
    selectors: Vec<usize>,

    // Which dim?
    width: Option<usize>,

    // Which dim?
    height: Option<usize>,

    // Which dim?
    channel: Option<usize>,
}

impl RankMapping {
    fn create(tensor: &Tensor) -> RankMapping {
        // TODO(emilk): add a heuristic here for the default
        RankMapping {
            width: Some(1),
            height: Some(0),
            channel: None,
            selectors: (2..tensor.num_dim()).collect(),
        }
    }
}

fn rank_mapping_ui(ui: &mut egui::Ui, rank_mapping: &mut RankMapping) {
    // TODO(emilk): Add a proper rank mapping GUI here
    if ui.button("transpose").clicked() {
        std::mem::swap(&mut rank_mapping.width, &mut rank_mapping.height);
    }
    ui.monospace(format!("{rank_mapping:?}"));
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
    ui.heading("Tensor viewer!");
    ui.monospace(format!("shape: {:?}", tensor.shape));
    ui.monospace(format!("dtype: {:?}", tensor.dtype));

    ui.collapsing("Rank Mapping", |ui| {
        rank_mapping_ui(ui, &mut state.rank_mapping);
    });
    color_mapping_ui(ui, &mut state.color_mapping);

    selectors_ui(ui, state, tensor);

    let color_mapping = &state.color_mapping;

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                let color_from_value =
                    |value: u8| color_mapping.color_from_normalized(value as f32 / 255.0);
                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::U16 => match re_tensor_ops::as_ndarray::<u16>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_u16(&tensor);
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                let color_from_value = |value: u16| {
                    color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::F32 => match re_tensor_ops::as_ndarray::<f32>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_f32(&tensor);
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                let color_from_value = |value: f32| {
                    color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ))
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());

                slice_ui(ui, &state.rank_mapping, slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },
    }
}

fn slice_ui<T: Copy>(
    ui: &mut egui::Ui,
    rank_mapping: &RankMapping,
    slice: ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) {
    ui.monospace(format!("Slice shape: {:?}", slice.shape()));
    if let Ok(slice) = slice.into_dimensionality::<Ix2>() {
        // Transpose depending on the rank-mapping. TODO: Handle this upstream.
        let image = if rank_mapping.height < rank_mapping.width {
            into_image(&slice, color_from_value)
        } else {
            into_image(&slice.t(), color_from_value)
        };
        image_ui(ui, image);
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            "Only 2D slices supported at the moment",
        );
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

fn image_ui(ui: &mut egui::Ui, image: ColorImage) {
    crate::profile_function!();
    // TODO(emilk): cache texture - don't create a new texture every frame
    let texture = ui
        .ctx()
        .load_texture("tensor_slice", image, egui::TextureFilter::Linear);
    egui::ScrollArea::both().show(ui, |ui| {
        ui.image(texture.id(), texture.size_vec2());
    });
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    for &dim_idx in &state.rank_mapping.selectors {
        let dim = &tensor.shape[dim_idx];
        let name = if dim.name.is_empty() {
            dim_idx.to_string()
        } else {
            dim.name.clone()
        };
        let len = dim.size;
        if len > 1 {
            let slice = state.selectors.entry(dim_idx).or_default();
            ui.add(egui::Slider::new(slice, 0..=len - 1).text(name));
        }
    }
}

fn tensor_range_f32(tensor: &ndarray::ArrayViewD<'_, f32>) -> (f32, f32) {
    crate::profile_function!();
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &value in tensor {
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn tensor_range_u16(tensor: &ndarray::ArrayViewD<'_, u16>) -> (u16, u16) {
    crate::profile_function!();
    let mut min = u16::MAX;
    let mut max = u16::MIN;
    for &value in tensor {
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn slicer(num_dim: usize, selectors: &ahash::HashMap<usize, u64>) -> Vec<ndarray::SliceInfoElem> {
    (0..num_dim)
        .map(|dim| {
            if let Some(selector) = selectors.get(&dim) {
                ndarray::SliceInfoElem::Index(*selector as _)
            } else {
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1,
                }
            }
        })
        .collect_vec()
}
