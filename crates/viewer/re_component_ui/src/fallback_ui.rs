use re_log_types::hash::Hash64;
use re_viewer_context::{
    UiLayout, ViewerContext,
    external::{re_chunk_store::LatestAtQuery, re_entity_db::EntityDb, re_log_types::EntityPath},
};

#[allow(clippy::too_many_arguments)]
pub fn fallback_component_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
    _cache_key: Option<Hash64>,
    component: &dyn arrow::array::Array,
) {
    re_ui::arrow_ui(ui, ui_layout, component);
}
