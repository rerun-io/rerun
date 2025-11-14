use re_data_ui::item_ui;
use re_types::{blueprint::components::TextLogColumnList, datatypes};
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_columns_singleline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    column_list: &mut MaybeMutRef<'_, TextLogColumnList>,
) -> egui::Response {
    ui.weak(match column_list.columns.len() {
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
            .columns
            .iter()
            .map(|column| match column {
                datatypes::TextLogColumn::Timeline(name) => {
                    item_ui::timeline_button(ctx, ui, &re_log_types::TimelineName::new(name))
                }
                _ => ui.strong(column.kind_name()),
            })
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(column_list) => {
            let columns = &mut column_list.columns;
            let mut any_edit = false;

            let mut remove = Vec::new();
            let dnd_res = egui_dnd::dnd(ui, "text_log_columns_dnd").show(
                columns.iter_mut().enumerate(),
                |ui, (idx, column), handle, _state| {
                    ui.horizontal(|ui| {
                        handle.ui(ui, |ui| {
                            ui.small_icon(
                                &re_ui::icons::DND_HANDLE,
                                Some(ui.visuals().text_color()),
                            );
                        });

                        egui::containers::Sides::new().shrink_left().show(
                            ui,
                            |ui| column_definition_ui(ctx, ui, column, &mut any_edit),
                            |ui| {
                                if ui
                                    .small_icon_button(&re_ui::icons::REMOVE, "remove column")
                                    .on_hover_text("Remove column")
                                    .clicked()
                                {
                                    remove.push(idx);
                                }
                            },
                        );
                    });
                },
            );

            if dnd_res.is_drag_finished() {
                any_edit = true;
                dnd_res.update_vec(columns);
            }
            // Skip removing if we dragged.
            else if !remove.is_empty() {
                any_edit = true;
                for i in remove.into_iter().rev() {
                    columns.remove(i);
                }
            }

            let mut response = ui
                .small_icon_button(&re_ui::icons::ADD, "add column")
                .on_hover_text("Add column");

            if response.clicked() {
                any_edit = true;
                let new_column = columns
                    .last()
                    .cloned()
                    .unwrap_or(re_types::datatypes::TextLogColumn::Body);

                columns.push(new_column);
            }

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
    column: &mut datatypes::TextLogColumn,
    any_edit: &mut bool,
) {
    egui::ComboBox::from_id_salt("column_types")
        .selected_text(column.kind_name())
        .show_ui(ui, |ui| {
            let timeline = if let datatypes::TextLogColumn::Timeline(name) = column {
                name.as_str().to_owned()
            } else {
                ctx.time_ctrl.timeline().name().to_string()
            };
            let mut selectable_value = |value: datatypes::TextLogColumn| {
                let text = value.kind_name();
                *any_edit |= ui.selectable_value(column, value, text).changed();
            };
            selectable_value(datatypes::TextLogColumn::Timeline(datatypes::Utf8::from(
                timeline,
            )));

            selectable_value(datatypes::TextLogColumn::EntityPath);

            selectable_value(datatypes::TextLogColumn::LogLevel);
            selectable_value(datatypes::TextLogColumn::Body);
        });

    if let datatypes::TextLogColumn::Timeline(name) = column {
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
