use re_log_types::{Tensor, TensorDataType};

use egui::{Color32, ColorImage};
use itertools::Itertools as _;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TensorViewState {
    /// maps dimenion to the slice of that dimension.
    selectors: ahash::HashMap<usize, u64>,
    rank_mapping: RankMapping,
}

impl TensorViewState {
    pub(crate) fn create(tensor: &re_log_types::Tensor) -> TensorViewState {
        Self {
            selectors: Default::default(),
            rank_mapping: RankMapping::create(tensor),
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
        // TODO: a heuristic
        RankMapping {
            width: Some(1),
            height: Some(0),
            channel: None,
            selectors: (2..tensor.num_dim()).collect(),
        }
    }
}

fn rank_mapping_ui(ui: &mut egui::Ui, rank_mapping: &mut RankMapping) {
    ui.label("TODO");
    if ui.button("transpose").clicked() {
        std::mem::swap(&mut rank_mapping.width, &mut rank_mapping.height);
    }
    ui.monospace(format!("{rank_mapping:?}"));
}

// ----------------------------------------------------------------------------

pub(crate) fn view_tensor(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    ui.heading("Tensor viewer!");
    ui.monospace(format!("shape: {:?}", tensor.shape));
    ui.monospace(format!("dtype: {:?}", tensor.dtype));

    ui.collapsing("Rank Mapping", |ui| {
        rank_mapping_ui(ui, &mut state.rank_mapping);
    });

    selectors_ui(ui, state, tensor);

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                let color_from_value = Color32::from_gray;
                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
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
                    let lum = egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=255.0,
                    )
                    .round() as u8;
                    Color32::from_gray(lum)
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
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
                    let lum =
                        egui::remap(value, tensor_min..=tensor_max, 0.0..=255.0).round() as u8;
                    Color32::from_gray(lum)
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
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
    slice: &ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) {
    ui.monospace(format!("Slice shape: {:?}", slice.shape()));

    if slice.ndim() == 2 {
        let image = into_image(rank_mapping, slice, color_from_value);
        image_ui(ui, image);
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            "Only 2D slices supported at the moment",
        );
    }
}

fn into_image<T: Copy>(
    rank_mapping: &RankMapping,
    slice: &ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) -> ColorImage {
    assert_eq!(slice.ndim(), 2);
    // what is height or what is width depends on the rank-mapping
    if rank_mapping.height < rank_mapping.width {
        let (height, width) = (slice.shape()[0], slice.shape()[1]);
        let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);
        assert_eq!(image.pixels.len(), slice.iter().count());
        for (pixel, value) in itertools::izip!(&mut image.pixels, slice) {
            *pixel = color_from_value(*value);
        }
        image
    } else {
        // transpose:
        let (width, height) = (slice.shape()[0], slice.shape()[1]);
        let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);
        assert_eq!(image.pixels.len(), slice.iter().count());
        for y in 0..height {
            for x in 0..width {
                image[(x, y)] = color_from_value(slice[[x, y]]);
            }
        }
        image
    }
}

fn image_ui(ui: &mut egui::Ui, image: ColorImage) {
    // TODO: cache texture - don't create a new texture every frame
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
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &value in tensor {
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn tensor_range_u16(tensor: &ndarray::ArrayViewD<'_, u16>) -> (u16, u16) {
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
