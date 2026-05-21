/// Paint the standard loading indicator for views whose required data is still being fetched.
pub fn paint_view_loading_indicator(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    view_rect: egui::Rect,
    any_missing_chunks: bool,
    recording: &re_entity_db::EntityDb,
) {
    let show_loading_indicator = (recording.is_downloading_manifest() || any_missing_chunks)
        && recording.can_fetch_chunks_from_redap();

    let loading_indicator_opacity = ui.ctx().animate_bool(
        ui.id().with(("loading_indicator", id_salt)),
        show_loading_indicator,
    );

    if 0.0 < loading_indicator_opacity {
        let reason = if recording.is_downloading_manifest() {
            "Downloading manifest from redap"
        } else {
            "Fetching chunks from redap"
        };

        re_ui::loading_indicator::paint_loading_indicator_inside(
            ui,
            egui::Align2::RIGHT_TOP,
            view_rect,
            loading_indicator_opacity,
            None,
            reason,
        );
    }
}
