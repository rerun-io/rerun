use re_data_ui::item_ui::timeline_button;

use re_log_types::TimelineName;
use re_types::blueprint::components::TimelineColumn;
use re_ui::{HasDesignTokens as _, UiExt as _};
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_columns_singleline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TimelineColumn>>,
) -> egui::Response {
    ui.weak(match columns.len() {
        1 => "1 column".to_owned(),
        l => format!("{l} columns"),
    })
}

pub fn edit_or_view_columns_multiline(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TimelineColumn>>,
) -> egui::Response {
    match columns {
        MaybeMutRef::Ref(columns) => columns
            .iter()
            .filter(|column| column.visible.into())
            .map(|column| timeline_button(ctx, ui, &TimelineName::new(&column.timeline)))
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(columns) => {
            let mut any_edit = false;

            let extra_columns = ctx
                .recording()
                .times_per_timeline()
                .timelines()
                .filter(|timeline| {
                    columns
                        .iter()
                        .all(|col| col.timeline.as_str() != timeline.name().as_str())
                })
                .map(|timeline| {
                    TimelineColumn(re_types::blueprint::datatypes::TimelineColumn {
                        visible: false.into(),
                        timeline: timeline.name().as_str().into(),
                    })
                })
                .collect::<Vec<_>>();

            columns.extend(extra_columns);

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
                    let dnd_res = egui_dnd::dnd(ui, "timeline_columns_dnd").show_sized(
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

                                let col = &mut **col;
                                egui::containers::Sides::new().shrink_left().show(
                                    ui,
                                    |ui| {
                                        if visible {
                                            timeline_button(
                                                ctx,
                                                ui,
                                                &TimelineName::new(&col.timeline),
                                            );
                                        } else {
                                            ui.weak(col.timeline.as_str());
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
