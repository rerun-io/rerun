use itertools::Itertools as _;
use re_log_types::EntityPath;
use re_log_types::hash::Hash64;
use re_sdk_types::datatypes::TensorData;
use re_sdk_types::{ComponentDescriptor, RowId};
use re_ui::UiExt as _;
use re_viewer_context::{TensorStats, TensorStatsCache, UiLayout, ViewerContext};

use super::EntityDataUi;

fn format_tensor_shape_single_line(tensor: &TensorData) -> String {
    let has_names = tensor
        .shape
        .iter()
        .enumerate()
        .any(|(dim_idx, _)| tensor.dim_name(dim_idx).is_some());
    tensor
        .shape
        .iter()
        .enumerate()
        .map(|(dim_idx, dim_len)| {
            format!(
                "{}{}",
                dim_len,
                if let Some(name) = tensor.dim_name(dim_idx) {
                    format!(" ({name})")
                } else {
                    String::new()
                }
            )
        })
        .join(if has_names { " × " } else { "×" })
}

impl EntityDataUi for re_sdk_types::components::TensorData {
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _entity_path: &EntityPath,
        _component_descriptor: &ComponentDescriptor,
        row_id: Option<RowId>,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();
        // RowId is enough for cache keying the tensor stats right now since you can't have more than one per row.
        let tensor_cache_key = row_id.map_or(Hash64::ZERO, Hash64::hash);
        tensor_ui(ctx, ui, ui_layout, tensor_cache_key, &self.0);
    }
}

pub fn tensor_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    tensor_cache_key: Hash64,
    tensor: &TensorData,
) {
    // See if we can convert the tensor to a GPU texture.
    // Even if not, we will show info about the tensor.
    let tensor_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_cache_key, tensor));

    if ui_layout.is_single_line() {
        ui.horizontal(|ui| {
            let text = format!(
                "{}, {}",
                tensor.dtype(),
                format_tensor_shape_single_line(tensor)
            );
            ui_layout.label(ui, text).on_hover_ui(|ui| {
                tensor_summary_ui(ui, tensor, &tensor_stats);
            });
        });
    } else {
        ui.vertical(|ui| {
            ui.set_min_width(100.0);
            tensor_summary_ui(ui, tensor, &tensor_stats);
        });
    }
}

pub fn tensor_summary_ui_grid_contents(
    ui: &mut egui::Ui,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
) {
    let TensorData { shape, names, .. } = tensor;

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
        if let Some(names) = names {
            for (name, size) in itertools::izip!(names, shape) {
                ui.label(format!("{name}={size}"));
            }
        } else {
            ui.label(format_tensor_shape_single_line(tensor));
        }
    });
    ui.end_row();

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
    if range != &Some(*finite_range) {
        ui.label("Finite data range").on_hover_text(
            "The finite values (ignoring all NaN & -Inf/+Inf) of the tensor range within these bounds"
        );
        let (min, max) = finite_range;
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
}

pub fn tensor_summary_ui(ui: &mut egui::Ui, tensor: &TensorData, tensor_stats: &TensorStats) {
    egui::Grid::new("tensor_summary_ui")
        .num_columns(2)
        .show(ui, |ui| {
            tensor_summary_ui_grid_contents(ui, tensor, tensor_stats);
        });
}
