mod model;

use self::model::{Model, ModelFilter, build_transform_cache_model};

use re_chunk_store::LatestAtQuery;
use re_ui::UiExt as _;
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::external::re_tf::transform_cache_snapshot;
use re_viewer_context::{StorageContext, TransformDatabaseStoreCache};

/// Compared to [`transform_cache_snapshot::FrameFilter`],
/// this adds a category for unlinked named frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum FrameVisibilityFilter {
    Implicit,
    #[default]
    Named,
    Unlinked,
    All,
}

/// Persistent UI state for the transform-cache dev panel.
#[derive(Default)]
pub(super) struct TransformCacheUiState {
    frame_filter: FrameVisibilityFilter,
    edge_filter: transform_cache_snapshot::EdgeFilter,
}

/// Draws the transform-cache dev-panel tab for the active recording and latest-at time.
pub(super) fn ui(
    ui: &mut egui::Ui,
    recording: &EntityDb,
    storage_context: &StorageContext<'_>,
    query: Option<LatestAtQuery>,
    state: &mut TransformCacheUiState,
) {
    re_tracing::profile_function!();

    let Some(query) = query else {
        ui.warning_label("No active timeline selected for the transform cache.");
        return;
    };

    let Some(caches) = storage_context.hub.store_caches(recording.store_id()) else {
        ui.label("No transform cache is available for this recording yet.");
        return;
    };

    let model = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
                frame_filter_ui(ui, state);
                ui.separator();

                edge_filter_ui(ui, state);
                ui.separator();

                let filter = ModelFilter {
                    frame_filter: state.frame_filter,
                    edge_filter: state.edge_filter,
                };
                let current_model = caches.memoizer(|cache: &mut TransformDatabaseStoreCache| {
                    build_transform_cache_model(recording, cache, &query, filter)
                });

                if current_model.snapshot.frames.is_empty() {
                    ui.warning_label("No transform frames match the current filter.");
                } else {
                    ui.info_label(format!(
                        "{}, {}, {}",
                        re_format::format_plural_s(current_model.num_trees(), "tree"),
                        re_format::format_plural_s(current_model.snapshot.frames.len(), "frame"),
                        re_format::format_plural_s(current_model.snapshot.edges.len(), "transform")
                    ));
                }
                ui.separator();

                // Center the recording name label horizontally to align it with the widgets' text.
                ui.horizontal_centered(|ui| {
                    let store_id = recording.store_id();
                    ui.label(format!(
                        "{} ({})",
                        store_id.application_id(),
                        store_id.recording_id()
                    ));

                    if current_model.any_missing_chunks {
                        ui.warning_label("Some chunks are missing");
                    }
                });

                current_model
            },
        )
        .inner;

    draw_transform_cache(ui, &model, state);
}

/// Draws the implicit/named frame filter controls.
fn frame_filter_ui(ui: &mut egui::Ui, state: &mut TransformCacheUiState) {
    ui.selectable_toggle(|ui| {
        ui.selectable_value(
            &mut state.frame_filter,
            FrameVisibilityFilter::Implicit,
            "Implicit",
        )
        .on_hover_text("Show only tf# frames derived from entity paths");
        ui.selectable_value(
            &mut state.frame_filter,
            FrameVisibilityFilter::Named,
            "Named",
        )
        .on_hover_text("Show explicitly named frames with transforms");
        ui.selectable_value(
            &mut state.frame_filter,
            FrameVisibilityFilter::Unlinked,
            "Unlinked",
        )
        .on_hover_text("Show named coordinate frames without transforms");
        ui.selectable_value(&mut state.frame_filter, FrameVisibilityFilter::All, "All")
            .on_hover_text("Show implicit, named, and unlinked frames");
    });
}

/// Draws the static/temporal edge filter controls.
fn edge_filter_ui(ui: &mut egui::Ui, state: &mut TransformCacheUiState) {
    ui.selectable_toggle(|ui| {
        ui.selectable_value(
            &mut state.edge_filter,
            transform_cache_snapshot::EdgeFilter::Static,
            "Static",
        )
        .on_hover_text("Show only static transforms");
        ui.selectable_value(
            &mut state.edge_filter,
            transform_cache_snapshot::EdgeFilter::Temporal,
            "Temporal",
        )
        .on_hover_text("Show only temporal transforms");
        ui.selectable_value(
            &mut state.edge_filter,
            transform_cache_snapshot::EdgeFilter::All,
            "All",
        )
        .on_hover_text("Show static and temporal transforms");
    });
}

/// Draws the transform-cache scene.
fn draw_transform_cache(ui: &mut egui::Ui, model: &Model, _state: &mut TransformCacheUiState) {
    ui.separator();
    egui::Frame::new()
        .fill(ui.tokens().faint_bg_color)
        .show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            // TODO(michael): Replace this placeholder with the zoomable transform-cache scene.
            ui.centered_and_justified(|ui| {
                ui.label(if model.snapshot.frames.is_empty() {
                    "No transform cache scene to display."
                } else {
                    "Transform cache scene placeholder."
                });
            });
        });
}
