use re_data_store::ObjPath;
use re_log_types::Tensor;

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
}

// ----------------------------------------------------------------------------
