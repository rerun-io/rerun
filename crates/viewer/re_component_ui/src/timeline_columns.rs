use re_data_ui::item_ui::timeline_button;
use re_log_types::TimelineName;
use re_sdk_types::blueprint::components::TimelineColumn;
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::visible_dnd::visible_dnd;

pub fn edit_or_view_columns_singleline(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TimelineColumn>>,
) -> egui::Response {
    ui.horizontal(|ui| {
        for col in columns.iter() {
            if *col.visible {
                timeline_button(ctx, ui, &TimelineName::new(&col.timeline));
            }
        }
    })
    .response
}

pub fn edit_or_view_columns_multiline(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TimelineColumn>>,
) -> egui::Response {
    match columns {
        MaybeMutRef::Ref(columns) => columns
            .iter()
            .filter(|col| col.visible.into())
            .map(|col| timeline_button(ctx, ui, &TimelineName::new(&col.timeline)))
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(columns) => {
            // Add new timelines to the end of the UI, if there is any edit
            // these will be written to the component.
            let extra_columns = ctx
                .recording()
                .timelines()
                .values()
                .filter(|timeline| {
                    columns
                        .iter()
                        .all(|col| col.timeline.as_str() != timeline.name().as_str())
                })
                .map(|timeline| {
                    TimelineColumn(re_sdk_types::blueprint::datatypes::TimelineColumn {
                        visible: false.into(),
                        timeline: timeline.name().as_str().into(),
                    })
                })
                .collect::<Vec<_>>();

            columns.extend(extra_columns);

            visible_dnd(
                ui,
                "timeline_columns_dnd",
                columns,
                |ui, col| {
                    timeline_button(ctx, ui, &TimelineName::new(&col.timeline));
                },
                |col| *col.visible,
                |col, v| col.visible = v.into(),
            )
        }
    }
}
