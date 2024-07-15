use std::collections::BTreeSet;

use egui_extras::{Column, TableRow};

use re_chunk_store::RowId;
use re_types_core::ComponentName;

/// Display a nicely configured table with the provided header ui, row ui, and row count.
pub(crate) fn table_ui(
    ui: &mut egui::Ui,
    sorted_components: &BTreeSet<ComponentName>,
    header_ui: impl FnOnce(egui_extras::TableRow<'_, '_>),
    row_count: usize,
    row_ui: impl FnMut(TableRow<'_, '_>),
) {
    re_tracing::profile_function!();

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            egui::Frame {
                inner_margin: egui::Margin::same(5.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                egui_extras::TableBuilder::new(ui)
                    .columns(
                        Column::auto_with_initial_suggestion(200.0).clip(true),
                        3 + sorted_components.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    //TODO(ab): remove when https://github.com/emilk/egui/pull/4817 is merged/released
                    .max_scroll_height(f32::INFINITY)
                    .auto_shrink([false, false])
                    .striped(true)
                    .header(re_ui::DesignTokens::table_line_height(), header_ui)
                    .body(|body| {
                        body.rows(re_ui::DesignTokens::table_line_height(), row_count, row_ui);
                    });
            });
        });
}

pub(crate) fn row_id_ui(ui: &mut egui::Ui, row_id: &RowId) {
    let s = row_id.to_string();
    let split_pos = s.char_indices().nth_back(5);

    ui.label(match split_pos {
        Some((pos, _)) => &s[pos..],
        None => &s,
    })
    .on_hover_text(s);
}
