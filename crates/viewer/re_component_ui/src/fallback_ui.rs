use re_viewer_context::{
    external::{
        re_chunk_store::{LatestAtQuery, RowId},
        re_entity_db::EntityDb,
        re_log_types::EntityPath,
    },
    UiLayout, ViewerContext,
};

#[allow(clippy::too_many_arguments)]
pub fn fallback_component_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
    _row_id: Option<RowId>,
    component: &dyn arrow::array::Array,
) {
    re_ui::arrow_ui(ui, ui_layout, component);
}
