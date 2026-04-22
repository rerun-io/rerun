use egui::{Frame, RichText, Ui};

use re_ui::UiExt as _;
use re_ui::egui_ext::card_layout::CardLayout;
use re_viewer_context::StoreViewContext;

use crate::DisplayRecordBatch;
use crate::datafusion_table_widget::{Columns, bool_value_at, find_row_batch};
use crate::display_record_batch::DisplayColumn;
use crate::re_table_utils::TableConfig;
use crate::table_blueprint::TableBlueprint;

pub struct FlagChangeEvent {
    pub row: u64,
    pub new_value: bool,
}

/// Shared parameters that are the same for every card in the grid.
struct CardConfig<'a> {
    table_config: &'a TableConfig,
    title_col_index: Option<usize>,
    table_blueprint: &'a TableBlueprint,
    flagging_enabled: bool,
}

/// Render the data as a card-based grid.
///
/// Returns a list of flag toggle changes that need to be applied to the underlying data.
#[expect(clippy::too_many_arguments)]
pub fn grid_ui(
    ctx: &StoreViewContext<'_>,
    ui: &mut Ui,
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    table_config: &TableConfig,
    table_blueprint: &TableBlueprint,
    num_table_rows: u64,
    flagging_enabled: bool,
) -> Vec<FlagChangeEvent> {
    let mut flag_changes = Vec::new();

    let tokens = ui.tokens();
    let card_min_width = tokens.table_grid_view_card_min_width;
    let card_spacing = tokens.table_grid_view_card_spacing;

    let inner_margin = egui::Margin::same(12);
    let card_frame = Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.table_grid_view_card_fill)
        .corner_radius(12.0);

    // Resolve the title column index once for all cards.
    let title_col_index = find_title_column_index(table_blueprint, columns, table_config);

    let card_config = CardConfig {
        table_config,
        title_col_index,
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
                    index as u64,
                    columns,
                    display_record_batches,
                    card_hovered,
                ));
            });
        });

    flag_changes
}

/// Render the content of a single card for the given table row.
///
/// This renders only the card interior — the frame is handled by [`CardLayout`].
fn card_content_ui(
    ctx: &StoreViewContext<'_>,
    config: &CardConfig<'_>,
    ui: &mut Ui,
    row_idx: u64,
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_hovered: bool,
) -> Option<FlagChangeEvent> {
    re_tracing::profile_function!();

    let &CardConfig {
        table_config,
        title_col_index,
        table_blueprint,
        flagging_enabled,
    } = config;

    let (display_record_batch, batch_index) =
        find_row_batch(display_record_batches, row_idx as usize)?;

    let mut flag_change_event = None;

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
        // Title row: title on the left, flag toggle on the right.
        egui::Sides::new().shrink_left().show(
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
                if flagging_enabled && let Some(flag_col) = &table_blueprint.flag_column {
                    let is_flagged =
                        bool_value_at(columns, display_record_batches, row_idx, flag_col)
                            .unwrap_or(false);

                    // Right-align the flag toggle.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if flag_button(ui, is_flagged, card_hovered).clicked() {
                            flag_change_event = Some(FlagChangeEvent {
                                row: row_idx,
                                new_value: !is_flagged,
                            });
                        }
                    });
                }
            },
        );

        // Footer: remaining visible columns as "label: value" pairs.
        // Tighter spacing (8px) between column name and its value,
        // wider spacing (20px) between separate columns.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let mut is_first_column = true;
            for col_idx in table_config.visible_column_indexes() {
                if Some(col_idx) == title_col_index {
                    continue; // already shown as the title
                }
                let col_name = columns
                    .columns
                    .get(col_idx)
                    .map_or_else(String::new, |c| c.display_name());

                if let Some(column) = display_record_batch.columns().get(col_idx) {
                    if !is_first_column {
                        // 20px total between columns: 8px item_spacing is already
                        // pending, so add the remaining 12px explicitly.
                        ui.add_space(12.0);
                    }
                    is_first_column = false;

                    ui.label(RichText::new(&col_name).monospace());
                    column.data_ui(ctx, ui, batch_index, None);
                }
            }
        });
    });

    flag_change_event
}

/// Find the column index to use as the card title.
///
/// If `grid_view_card_title` is set in the blueprint, uses that column.
/// Otherwise falls back to the first visible string-typed column.
fn find_title_column_index(
    table_blueprint: &TableBlueprint,
    columns: &Columns<'_>,
    table_config: &TableConfig,
) -> Option<usize> {
    if let Some(title_col_name) = &table_blueprint.grid_view_card_title {
        for col_idx in table_config.visible_column_indexes() {
            if columns
                .columns
                .get(col_idx)
                .is_some_and(|c| c.display_name() == *title_col_name)
            {
                return Some(col_idx);
            }
        }
    }

    // Fallback: first visible column that has string data.
    for col_idx in table_config.visible_column_indexes() {
        if let Some(col) = columns.columns.get(col_idx)
            && matches!(
                col.desc,
                re_sorbet::ColumnDescriptorRef::Component(c) if c.store_datatype == arrow::datatypes::DataType::Utf8
            )
        {
            return Some(col_idx);
        }
    }

    None
}

/// A flag toggle button with progressive-disclosure styling.
///
/// Three visual tiers based on hover context:
/// - **Idle** (mouse away from card): transparent bg, muted icon — flag "melts" into the card.
/// - **Card hovered**: subtle bg appears, icon becomes legible — flag is *revealed*.
/// - **Flag hovered**: stronger bg, same icon — flag is clearly *actionable*.
///
/// When toggled on the flag is always visible (orange) so the user can see their selection
/// at a glance, with the same three-tier brightness progression on hover.
#[expect(clippy::fn_params_excessive_bools)]
fn flag_button(ui: &mut Ui, is_flagged: bool, card_hovered: bool) -> egui::Response {
    let tokens = ui.tokens();

    let size = egui::vec2(30.0, 24.0);
    let icon_size = egui::vec2(14.0, 14.0);

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            is_flagged,
            "Flag",
        )
    });

    if ui.is_rect_visible(rect) {
        let flag_hovered = response.hovered();

        let (bg, icon_tint) = if is_flagged {
            let bg = if flag_hovered {
                tokens.flag_toggled_bg_hover
            } else if card_hovered {
                tokens.flag_toggled_bg_card_hover
            } else {
                tokens.flag_toggled_bg
            };
            (bg, tokens.flag_toggled_icon)
        } else {
            let bg = if flag_hovered {
                tokens.flag_untoggled_bg_hover
            } else if card_hovered {
                tokens.flag_untoggled_bg_card_hover
            } else {
                tokens.flag_untoggled_bg
            };
            let icon_tint = if flag_hovered || card_hovered {
                tokens.flag_untoggled_icon_hover
            } else {
                tokens.flag_untoggled_icon
            };
            (bg, icon_tint)
        };

        if bg.a() > 0 {
            let rounding = 4.0;
            ui.painter().rect_filled(rect, rounding, bg);
        }

        let icon = if is_flagged {
            &re_ui::icons::FLAG_TOGGLED
        } else {
            &re_ui::icons::FLAG_UNTOGGLED
        };

        let icon_rect = egui::Rect::from_center_size(rect.center(), icon_size);
        icon.as_image().tint(icon_tint).paint_at(ui, icon_rect);
    }
    response
}
