use re_types::blueprint::{components::TextLogColumnList, datatypes::TextLogColumnKind};
use re_ui::{HasDesignTokens as _, UiExt as _};
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_columns_singleline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    column_list: &mut MaybeMutRef<'_, TextLogColumnList>,
) -> egui::Response {
    ui.weak(match column_list.text_log_columns.len() {
        1 => "1 column".to_owned(),
        l => format!("{l} columns"),
    })
}

pub fn edit_or_view_columns_multiline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    column_list: &mut MaybeMutRef<'_, TextLogColumnList>,
) -> egui::Response {
    match column_list {
        MaybeMutRef::Ref(column_list) => column_list
            .text_log_columns
            .iter()
            .filter(|column| column.visible.into())
            .map(|column| ui.strong(column.kind.kind_name()))
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(column_list) => {
            let columns = &mut column_list.text_log_columns;
            let mut any_edit = false;

            const ITEM_SPACING: f32 = 8.0;
            let egui::InnerResponse { mut response, .. } = egui::Frame::new()
                .corner_radius(ui.visuals().menu_corner_radius)
                .fill(ui.visuals().tokens().text_edit_bg_color)
                .inner_margin(egui::Margin {
                    left: ITEM_SPACING as i8,
                    right: ITEM_SPACING as i8,
                    top: ITEM_SPACING as i8,
                    bottom: (ITEM_SPACING * 0.5) as i8,
                })
                .show(ui, |ui| {
                    let text_height = ui
                        .style()
                        .text_styles
                        .get(&egui::TextStyle::Body)
                        .map(|s| s.size)
                        .unwrap_or(0.0);
                    let sz = egui::vec2(ui.max_rect().size().x, ITEM_SPACING + text_height);
                    let dnd_res = egui_dnd::dnd(ui, "text_log_columns_dnd").show_sized(
                        columns.iter_mut().enumerate(),
                        sz,
                        |ui, (_idx, col), handle, _state| {
                            ui.horizontal(|ui| {
                                handle.ui(ui, |ui| {
                                    ui.small_icon(
                                        &re_ui::icons::DND_HANDLE,
                                        Some(ui.visuals().text_color()),
                                    );
                                });

                                let visible = col.visible.0;

                                egui::containers::Sides::new().shrink_left().show(
                                    ui,
                                    |ui| {
                                        let column: &mut TextLogColumnKind = &mut col.kind;
                                        let name = column.kind_name();
                                        if visible {
                                            ui.strong(name);
                                        } else {
                                            ui.weak(name);
                                        }
                                    },
                                    |ui| {
                                        any_edit |= ui
                                            .visibility_toggle_button(&mut col.visible.0)
                                            .changed();
                                    },
                                );
                            });
                        },
                    );

                    if dnd_res.is_drag_finished() {
                        any_edit = true;
                        dnd_res.update_vec(columns);
                    }
                });

            if any_edit {
                response.mark_changed();
            }

            response
        }
    }
}
