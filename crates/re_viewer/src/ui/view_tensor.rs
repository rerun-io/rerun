use re_data_store::ObjPath;
use re_log_types::Tensor;

use egui::Color32;
use itertools::Itertools as _;

use crate::misc::ViewerContext;

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
    ui.monospace(format!("{rank_mapping:?}"));
}

// ----------------------------------------------------------------------------

pub(crate) fn view_tensor(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut TensorViewState,
    space: Option<&ObjPath>,
    tensor: &Tensor,
) {
    ui.heading("Tensor viewer!");
    ui.monospace(format!("shape: {:?}", tensor.shape));
    ui.monospace(format!("dtype: {:?}", tensor.dtype));

    ui.collapsing("Rank Mapping", |ui| {
        rank_mapping_ui(ui, &mut state.rank_mapping);
    });

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

    if let Ok(tensor) = re_tensor_ops::as_ndarray::<f32>(tensor) {
        let slice_info_elems = (0..tensor.ndim())
            .map(|dim| {
                if let Some(selector) = state.selectors.get(&dim) {
                    ndarray::SliceInfoElem::Index(*selector as _)
                } else {
                    ndarray::SliceInfoElem::Slice {
                        start: 0,
                        end: None,
                        step: 1,
                    }
                }
            })
            .collect_vec();
        let slice = tensor.slice(slice_info_elems.as_slice());
        ui.monospace(format!("Slice shape: {:?}", slice.shape()));

        if slice.ndim() == 2 {
            assert_eq!(slice.shape().len(), 2);
            let (height, width) = (slice.shape()[0], slice.shape()[1]); // TODO: what is height or what is width should come from the rank-mapping

            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for &value in &slice {
                min = min.min(value);
                max = max.max(value);
            }

            ui.monospace(format!("Data range: [{min} - {max}]"));

            let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);

            assert_eq!(image.pixels.len(), slice.iter().count());
            if false {
                for (pixel, value) in itertools::izip!(&mut image.pixels, &slice) {
                    let lum = egui::remap(*value, min..=max, 0.0..=255.0).round() as u8;
                    *pixel = Color32::from_gray(lum);
                }
            } else {
                // slower, but does range sanity checking
                for y in 0..height {
                    for x in 0..width {
                        let value = slice[[y, x]];
                        let lum = egui::remap(value, min..=max, 0.0..=255.0).round() as u8;
                        image[(x, y)] = Color32::from_gray(lum);
                    }
                }
            }

            if ui.button("Save image").clicked() {
                let image = image::RgbaImage::from_raw(
                    width as _,
                    height as _,
                    bytemuck::cast_slice(&image.pixels).to_vec(),
                )
                .unwrap();
                let image = image::DynamicImage::ImageRgba8(image);
                let path = "tensor_slice.png";
                image.save(path).unwrap();
                re_log::info!("Saved to {path:?}");
            }

            let texture = ui
                .ctx()
                .load_texture("tensor_slice", image, egui::TextureFilter::Linear); // TODO: cache - don't call every frame

            ui.image(texture.id(), texture.size_vec2());
        }
    }
}

// ----------------------------------------------------------------------------
