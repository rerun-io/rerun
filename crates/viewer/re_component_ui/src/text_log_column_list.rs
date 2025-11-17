use re_data_ui::item_ui;
use re_types::{blueprint::components::TextLogColumnList, datatypes};
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
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    column_list: &mut MaybeMutRef<'_, TextLogColumnList>,
) -> egui::Response {
    match column_list {
        MaybeMutRef::Ref(column_list) => column_list
            .text_log_columns
            .iter()
            .filter(|column| column.visible.into())
            .map(|column| match &column.kind {
                datatypes::TextLogColumnKind::Timeline(name) => {
                    item_ui::timeline_button(ctx, ui, &re_log_types::TimelineName::new(name))
                }
                _ => ui.strong(column.kind.kind_name()),
            })
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(column_list) => {
            let columns = &mut column_list.text_log_columns;
            let mut any_edit = false;

            const ITEM_SPACING: f32 = 8.0;
            let egui::InnerResponse { mut response, .. } = egui::Frame::new()
                .corner_radius(ui.visuals().menu_corner_radius)
                .fill(ui.visuals().tokens().text_edit_bg_color)
                .inner_margin(egui::Margin::symmetric(
                    ITEM_SPACING as i8,
                    (ITEM_SPACING * 0.5) as i8,
                ))
                .show(ui, |ui| {
                    let dnd_res =
                        egui_dnd::dnd(ui, "text_log_columns_dnd").show_custom(|ui, item_iter| {
                            for (idx, col) in columns.iter_mut().enumerate() {
                                let id = egui::Id::new((idx, &*col));
                                let space_content = |ui: &mut egui::Ui, space| {
                                    ui.set_min_size(space + egui::vec2(0.0, ITEM_SPACING));
                                };

                                item_iter.space_before(ui, id, space_content);

                                item_iter.next(ui, id, idx, false, |ui, item| {
                                    let dragged = item.state.dragged;
                                    if !dragged {
                                        ui.add_space(ITEM_SPACING * 0.5);
                                    }
                                    let res = item.ui(ui, |ui, handle, _state| {
                                        ui.horizontal(|ui| {
                                            handle.ui(ui, |ui| {
                                                ui.small_icon(
                                                    &re_ui::icons::DND_HANDLE,
                                                    Some(ui.visuals().text_color()),
                                                );
                                            });

                                            let visible = col.visible.0;

                                            let (_, changed) =
                                                egui::containers::Sides::new().shrink_left().show(
                                                    ui,
                                                    |ui| {
                                                        column_definition_ui(
                                                            ctx,
                                                            ui,
                                                            &mut col.kind,
                                                            visible,
                                                            &mut any_edit,
                                                        );
                                                    },
                                                    |ui| {
                                                        ui.visibility_toggle_button(
                                                            &mut col.visible.0,
                                                        )
                                                        .changed()
                                                    },
                                                );

                                            any_edit |= changed;
                                        });
                                    });
                                    if !dragged {
                                        ui.add_space(ITEM_SPACING * 0.5);
                                    }
                                    res
                                });

                                item_iter.space_after(ui, id, space_content);
                            }
                        });

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

fn column_definition_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    column: &mut datatypes::TextLogColumnKind,
    visible: bool,
    any_edit: &mut bool,
) {
    let name = match column {
        datatypes::TextLogColumnKind::Timeline(_) => "Timeline:",
        _ => column.kind_name(),
    };
    if visible {
        ui.strong(name);
    } else {
        ui.weak(name);
    }

    if let datatypes::TextLogColumnKind::Timeline(name) = column {
        egui::ComboBox::from_id_salt("column_timeline_name")
            .selected_text(name.as_str())
            .show_ui(ui, |ui| {
                for timeline in ctx.recording().times_per_timeline().timelines() {
                    *any_edit |= ui
                        .selectable_value(
                            name,
                            datatypes::Utf8::from(timeline.name().as_str()),
                            timeline.name().as_str(),
                        )
                        .changed();
                }
            });
    }
}
