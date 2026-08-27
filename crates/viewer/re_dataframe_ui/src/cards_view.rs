use std::str::FromStr as _;

use arrow::array::Array as _;
use egui::{Frame, RichText, Ui};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_component_ui::TABLE_FLAG_VARIANT;
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::egui_ext::card_layout::CardLayout;
use re_ui::{UiExt as _, UiLayout};
use re_viewer_context::{AppContext, VariantName, ViewStates};

use crate::DisplayRecordBatch;
use crate::datafusion_table_widget::{Columns, find_row_batch, resolve_recording_for_row};
use crate::display_record_batch::DisplayColumn;
use crate::preview_renderer::RecordingPreviewRenderer;
use crate::re_table_utils::UiTableConfig;
use crate::table_blueprint::TableBlueprint;

/// Height of the segment preview area inside each card.
const PREVIEW_HEIGHT: f32 = 200.0;

pub struct FlagChangeEvent {
    pub row: u64,
    pub new_value: bool,
}

/// Shared parameters that are the same for every card in the grid.
struct CardConfig<'a> {
    table_config: &'a UiTableConfig,
    title_col_index: Option<usize>,
    url_col_index: Option<usize>,
    segment_preview_column: Option<&'a ColumnName>,
    table_blueprint: &'a TableBlueprint,
    flagging_enabled: bool,
}

/// Render the data using the card layout.
///
/// Returns a list of flag toggle changes that need to be applied to the underlying data.
pub fn cards_ui(
    ctx: &AppContext<'_>,
    ui: &mut Ui,
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    table_config: &UiTableConfig,
    table_blueprint: &TableBlueprint,
    view_renderer: Option<&RecordingPreviewRenderer<'_>>,
    view_states: &mut ViewStates,
    num_table_rows: u64,
    flagging_enabled: bool,
) -> Vec<FlagChangeEvent> {
    let mut flag_changes = Vec::new();

    // Blueprint fields are expected to be resolved upstream via `TableBlueprint::apply_heuristics`,
    // so we only need a direct name lookup here.
    let title_col_index = table_blueprint
        .grid_view_card_title
        .as_ref()
        .and_then(|name| lookup_column(columns, name, "Title"));
    let url_col_index = table_blueprint
        .url_column
        .as_ref()
        .and_then(|name| lookup_column(columns, name, "URL"));
    let segment_preview_column = table_blueprint
        .segment_preview_column
        .as_ref()
        .filter(|name| columns.index_by_physical_name(name).is_some());

    let tokens = ui.tokens();
    let card_spacing = tokens.table_grid_view_card_spacing;

    // Scale the card width with the number of views so each view keeps roughly the same
    // footprint as a single-view card.
    let num_views = view_renderer.map_or(1, |r| r.num_views()).max(1);
    let card_min_width = tokens.table_grid_view_card_min_width * num_views as f32;

    let inner_margin = egui::Margin::same(tokens.table_grid_view_card_inner_margin as i8);
    let card_frame = Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.table_grid_view_card_fill)
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    let card_config = CardConfig {
        table_config,
        title_col_index,
        url_col_index,
        segment_preview_column,
        table_blueprint,
        flagging_enabled,
    };

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .content_margin(egui::Margin::same(card_spacing as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(card_spacing, card_spacing);

            CardLayout::uniform(
                num_table_rows as usize,
                card_min_width + card_spacing,
                card_frame,
            )
            .all_rows_use_available_width(false)
            .hover_fill(tokens.table_grid_view_card_hover_fill)
            .show(ui, |ui, index, card_hovered| {
                flag_changes.extend(card_content_ui(
                    ctx,
                    &card_config,
                    ui,
                    view_renderer,
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
fn lookup_column(columns: &Columns<'_>, name: &ColumnName, kind: &str) -> Option<usize> {
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
    view_renderer: Option<&RecordingPreviewRenderer<'_>>,
    view_states: &mut ViewStates,
    row_idx: u64,
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_hovered: bool,
) -> Option<FlagChangeEvent> {
    re_tracing::profile_function!();

    let &CardConfig {
        table_config,
        title_col_index,
        url_col_index,
        segment_preview_column,
        table_blueprint,
        flagging_enabled,
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
                if let Some(flag_column) = &table_blueprint.flag_column
                    && let Some(column_index) = columns.index_by_physical_name(flag_column)
                {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(column) = display_record_batch.columns().get(column_index)
                            && let Some(edited) = column.data_ui(
                                ctx,
                                ui,
                                batch_index,
                                None,
                                UiLayout::List,
                                Some(VariantName::from_static_str(TABLE_FLAG_VARIANT)),
                                flagging_enabled,
                            )
                        {
                            let new_value = edited
                                .downcast_array_ref::<arrow::array::BooleanArray>()
                                .and_then(|edited| {
                                    (!edited.is_empty() && !edited.is_null(0))
                                        .then(|| edited.value(0))
                                });

                            if let Some(new_value) = new_value {
                                flag_change_event = Some(FlagChangeEvent {
                                    row: row_idx,
                                    new_value,
                                });
                            }
                        }
                    });
                }
            },
        );

        // Segment preview if any.
        // TODO(RR-4510): loading indication if we're not ready to draw
        if let Some(renderer) = view_renderer
            && let Some(preview_column) = segment_preview_column
        {
            let preview_state = view_states.preview_state.get_or_insert_default();
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), PREVIEW_HEIGHT),
                egui::Sense::hover(),
            );

            let recording = resolve_recording_for_row(
                ctx,
                preview_column,
                columns,
                display_record_batches,
                row_idx,
                &mut preview_state.requested_uris,
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            renderer.show_preview(
                ctx,
                &mut child_ui,
                row_idx,
                card_hovered,
                recording,
                view_states,
            );
        }

        ui.horizontal_wrapped(|ui| {
            for column_name in table_config.visible_column_names() {
                let Some(col_idx) = columns.index_by_physical_name(column_name) else {
                    continue;
                };
                if Some(col_idx) == title_col_index {
                    continue; // already shown as the title
                }
                let col_name = columns.columns[col_idx].display_name();

                if let Some(column) = display_record_batch.columns().get(col_idx) {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(RichText::new(&col_name).monospace());
                    ui.spacing_mut().item_spacing.x = 20.0;
                    column.data_ui(ctx, ui, batch_index, None, UiLayout::Inline, None, false);
                }
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
