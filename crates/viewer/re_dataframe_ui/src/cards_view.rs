use std::str::FromStr as _;

use arrow::array::Array as _;
use egui::{Frame, RichText, Ui};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sdk_types::blueprint::components::{ColumnName, TableCellKind, TableLayoutKind};
use re_ui::{UiExt as _, UiLayout};
use re_viewer_context::{AppContext, ViewStates};

use crate::DisplayRecordBatch;
use crate::blueprint::CardLayout;
use crate::datafusion_table_widget::{DataColumns, ResolvedFlagColumn, find_row_batch};
use crate::display_record_batch::DisplayColumn;
use crate::preview_renderer::RecordingPreviewRenderer;

/// Height of the segment preview area inside each card.
const PREVIEW_HEIGHT: f32 = 200.0;

pub struct FlagChangeEvent {
    pub row: u64,
    pub physical_column: ColumnName,
    pub new_value: bool,
}

/// Shared parameters that are the same for every card in the grid.
struct CardConfig<'a> {
    title_col_index: Option<usize>,
    url_col_index: Option<usize>,
    card_layout: &'a CardLayout<'a>,
    flag_column: Option<&'a ColumnName>,
    flag_editable: bool,
}

/// Render the data using the card layout.
///
/// Returns a list of flag toggle changes that need to be applied to the underlying data.
pub fn cards_ui(
    ctx: &AppContext<'_>,
    ui: &mut Ui,
    columns: &DataColumns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_layout: &CardLayout<'_>,
    view_renderers: &[RecordingPreviewRenderer<'_>],
    view_states: &mut ViewStates,
    num_table_rows: u64,
    editable_flag_columns: &[ResolvedFlagColumn],
) -> Vec<FlagChangeEvent> {
    let mut flag_changes = Vec::new();

    // Blueprint fields are expected to be resolved upstream via `TableBlueprint::apply_heuristics`,
    // so we only need a direct name lookup here.
    let title_col_index = card_layout
        .title()
        .and_then(|name| lookup_column(columns, name, "Title"));
    let url_col_index = card_layout
        .link()
        .and_then(|name| lookup_column(columns, name, "Link"));

    let tokens = ui.tokens();
    let card_spacing = tokens.table_grid_view_card_spacing;

    // Scale the card width with the number of views so each view keeps roughly the same
    // footprint as a single-view card.
    let max_num_views_horizontal = view_renderers
        .iter()
        .map(RecordingPreviewRenderer::num_views)
        .max()
        .unwrap_or(1);
    let card_min_width = tokens.table_grid_view_card_min_width * max_num_views_horizontal as f32;

    let inner_margin = egui::Margin::same(tokens.table_grid_view_card_inner_margin as i8);
    let card_frame = Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    let flag_column = card_layout.flag().map(|field| field.physical_name());
    let flag_editable = flag_column.is_some_and(|field| {
        editable_flag_columns
            .iter()
            .any(|column| column.physical_name == *field)
    });
    let card_config = CardConfig {
        title_col_index,
        url_col_index,
        card_layout,
        flag_column,
        flag_editable,
    };

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .content_margin(egui::Margin::same(card_spacing as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(card_spacing, card_spacing);

            re_ui::egui_ext::card_layout::CardLayout::uniform(
                num_table_rows as usize,
                card_min_width + card_spacing,
                card_frame,
            )
            .all_rows_use_available_width(false)
            .hover_fill(tokens.card_hover_fill)
            .hover_stroke(tokens.card_hover_stroke)
            .show(ui, |ui, index, card_hovered| {
                flag_changes.extend(card_content_ui(
                    ctx,
                    &card_config,
                    ui,
                    view_renderers,
                    view_states,
                    index as u64,
                    columns,
                    display_record_batches,
                    card_hovered,
                ));
            });
        });

    flag_changes
}

/// Look up a column by its physical name, warning once if it is missing.
fn lookup_column(columns: &DataColumns<'_>, name: &ColumnName, kind: &str) -> Option<usize> {
    let found = columns.index_by_physical_name(name);
    if found.is_none() {
        re_log::warn_once!("{kind} column {name:?} was not found in the table.");
    }
    found
}

/// Render the content of a single card for the given table row.
///
/// This renders only the card interior — the frame is handled by [`CardLayout`].
fn card_content_ui(
    ctx: &AppContext<'_>,
    config: &CardConfig<'_>,
    ui: &mut Ui,
    view_renderers: &[RecordingPreviewRenderer<'_>],
    view_states: &mut ViewStates,
    row_idx: u64,
    data_columns: &DataColumns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_hovered: bool,
) -> Option<FlagChangeEvent> {
    re_tracing::profile_function!();

    let &CardConfig {
        title_col_index,
        url_col_index,
        card_layout,
        flag_column,
        flag_editable,
    } = config;

    let (display_record_batch, batch_index) =
        find_row_batch(display_record_batches, row_idx as usize)?;

    let mut flag_change_event = None;

    // Register a click sense over the whole card area *before* drawing content so that
    // interactive child widgets (flag button, etc.) take click priority.
    let card_click_response = ui.interact(
        ui.max_rect(),
        ui.id().with(("card_click", row_idx)),
        egui::Sense::click(),
    );

    // Read the title value for this row from the pre-resolved title column.
    let title_text = title_col_index.and_then(|idx| {
        if let Some(DisplayColumn::Component(comp)) = display_record_batch.columns().get(idx) {
            comp.string_value_at(batch_index)
        } else {
            None
        }
    });

    // CardLayout calls us inside a horizontal row — we need vertical layout for card content.
    ui.vertical(|ui| {
        ui.set_max_width(ui.available_width());

        // Title row: title on the left (truncate if needed), flag toggle on the right.
        egui::Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                if let Some(title_text) = title_text {
                    ui.label(
                        RichText::new(title_text)
                            .size(14.0)
                            .color(ui.tokens().text_default),
                    );
                }
            },
            |ui| {
                if let Some(flag_column) = flag_column
                    && let Some(column_index) = data_columns.index_by_physical_name(flag_column)
                {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(column) = display_record_batch.columns().get(column_index) {
                            let cell_kind = card_layout
                                .fields()
                                .iter()
                                .find(|field| field.physical_name() == flag_column)
                                .map_or(TableCellKind::Auto, |field| {
                                    field.value_resolved_cell_kind(Some(column), batch_index)
                                });

                            if let Some(edited) = column.data_ui(
                                ctx,
                                ui,
                                batch_index,
                                None,
                                UiLayout::List,
                                cell_kind,
                                flag_editable,
                            ) {
                                let new_value = edited
                                    .downcast_array_ref::<arrow::array::BooleanArray>()
                                    .and_then(|edited| {
                                        (!edited.is_empty() && !edited.is_null(0))
                                            .then(|| edited.value(0))
                                    });

                                if let Some(new_value) = new_value {
                                    flag_change_event = Some(FlagChangeEvent {
                                        row: row_idx,
                                        physical_column: flag_column.clone(),
                                        new_value,
                                    });
                                }
                            }
                        }
                    });
                }
            },
        );

        // TODO(RR-4510): loading indication if we're not ready to draw
        for renderer in view_renderers {
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), PREVIEW_HEIGHT),
                egui::Sense::hover(),
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            renderer.show_preview_for_row(
                ctx,
                &mut child_ui,
                row_idx,
                card_hovered,
                display_record_batches,
                view_states,
            );
        }

        ui.horizontal_wrapped(|ui| {
            for field in card_layout.fields() {
                let Some(col_idx) = data_columns.index_by_physical_name(field.physical_name())
                else {
                    continue;
                };
                if !field.is_visible(TableLayoutKind::Cards) {
                    continue;
                }
                let Some(column) = display_record_batch.columns().get(col_idx) else {
                    continue;
                };

                // Skip preview and flag cells as they are handled separately.
                let cell_kind = field.value_resolved_cell_kind(Some(column), batch_index);
                if matches!(cell_kind, TableCellKind::Preview | TableCellKind::Flag) {
                    continue;
                }

                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new(field.display_name()).monospace());
                ui.spacing_mut().item_spacing.x = 20.0;
                column.data_ui(
                    ctx,
                    ui,
                    batch_index,
                    None,
                    UiLayout::Inline,
                    cell_kind,
                    false,
                );
            }
        });
    });

    if card_click_response.clicked()
        && let Some(idx) = url_col_index
        && let Some(DisplayColumn::Component(comp)) = display_record_batch.columns().get(idx)
        && let Some(url_str) = comp.string_value_at(batch_index)
        && re_uri::RedapUri::from_str(&url_str).is_ok()
    {
        ui.open_url(egui::OpenUrl::same_tab(url_str));
    }

    flag_change_event
}
