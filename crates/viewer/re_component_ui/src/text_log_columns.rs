use re_sdk_types::blueprint::components::TextLogColumn;
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::visible_dnd::visible_dnd;

pub fn edit_or_view_columns_singleline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TextLogColumn>>,
) -> egui::Response {
    ui.horizontal(|ui| {
        let mut first = true;
        for col in columns.iter() {
            if !*col.visible {
                continue;
            }

            if first {
                first = false;
            } else {
                ui.separator();
            }

            ui.strong(col.kind.name());
        }
    })
    .response
}

pub fn edit_or_view_columns_multiline(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    columns: &mut MaybeMutRef<'_, Vec<TextLogColumn>>,
) -> egui::Response {
    match columns {
        MaybeMutRef::Ref(columns) => columns
            .iter()
            .filter(|column| column.visible.into())
            .map(|column| ui.strong(column.kind.name()))
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| ui.weak("Empty")),
        MaybeMutRef::MutRef(columns) => visible_dnd(
            ui,
            "text_log_columns_dnd",
            columns,
            |ui, col| {
                let name = col.kind.name();
                if *col.visible {
                    ui.strong(name);
                } else {
                    ui.weak(name);
                }
            },
            |col| *col.visible,
            |col, v| col.visible = v.into(),
        ),
    }
}
