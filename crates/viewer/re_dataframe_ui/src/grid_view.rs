use std::str::FromStr as _;

use ahash::HashSet;
use egui::{Frame, RichText, Ui};

use re_ui::egui_ext::card_layout::CardLayout;
use re_ui::{UiExt as _, UiLayout};
use re_viewer_context::{StoreViewContext, ViewStates};

use crate::DisplayRecordBatch;
use crate::datafusion_table_widget::{
    Columns, bool_value_at, find_row_batch, resolve_recording_for_row,
};
use crate::display_record_batch::DisplayColumn;
use crate::preview_renderer::RecordingPreviewRenderer;
use crate::re_table_utils::TableConfig;
use crate::table_blueprint::TableBlueprint;

/// Height of the segment preview area inside each card.
const PREVIEW_HEIGHT: f32 = 200.0;

pub struct FlagChangeEvent {
    pub row: u64,
    pub new_value: bool,
}

/// Shared parameters that are the same for every card in the grid.
struct CardConfig<'a> {
    table_config: &'a TableConfig,
    title_col_index: Option<usize>,
    url_col_index: Option<usize>,
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
    view_renderer: Option<&RecordingPreviewRenderer<'_>>,
    view_states: &mut ViewStates,
    num_table_rows: u64,
    flagging_enabled: bool,
) -> Vec<FlagChangeEvent> {
    let mut already_requested_uris = HashSet::default();
    let mut flag_changes = Vec::new();

    // Blueprint fields are expected to be resolved upstream via `TableBlueprint::apply_heuristics`,
    // so we only need a direct name lookup here.
    let title_col_index = table_blueprint
        .grid_view_card_title
        .as_deref()
        .and_then(|name| lookup_column(columns, name, "Title"));
    let url_col_index = table_blueprint
        .url_column
        .as_deref()
        .and_then(|name| lookup_column(columns, name, "URL"));

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
                    &mut already_requested_uris,
                    index as u64,
                    columns,
                    display_record_batches,
                    card_hovered,
                ));
            });
        });

    flag_changes
}

/// Look up a column by its display name, warning once if it is missing.
fn lookup_column(columns: &Columns<'_>, name: &str, kind: &str) -> Option<usize> {
    let found = columns.find_index_by_display_name(name);
    if found.is_none() {
        re_log::warn_once!("{kind} column {name:?} was not found in the table.");
    }
    found
}

/// Render the content of a single card for the given table row.
///
/// This renders only the card interior — the frame is handled by [`CardLayout`].
#[expect(clippy::too_many_arguments)]
fn card_content_ui(
    ctx: &StoreViewContext<'_>,
    config: &CardConfig<'_>,
    ui: &mut Ui,
    view_renderer: Option<&RecordingPreviewRenderer<'_>>,
    view_states: &mut ViewStates,
    already_requested_uris: &mut HashSet<re_uri::DatasetSegmentUri>,
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

        // Segment preview if any.
        // TODO(RR-4510): loading indication if we're not ready to draw
        if let Some(renderer) = view_renderer
            && let Some(preview_column) = table_blueprint.segment_preview_column.as_deref()
        {
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
                already_requested_uris,
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            renderer.show_preview(ctx.app_ctx, &mut child_ui, row_idx, recording, view_states);
        }

        ui.horizontal_wrapped(|ui| {
            for col_idx in table_config.visible_column_indexes() {
                if Some(col_idx) == title_col_index {
                    continue; // already shown as the title
                }
                let col_name = columns
                    .columns
                    .get(col_idx)
                    .map_or_else(String::new, |c| c.display_name());

                if let Some(column) = display_record_batch.columns().get(col_idx) {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(RichText::new(&col_name).monospace());
                    ui.spacing_mut().item_spacing.x = 20.0;
                    column.data_ui(ctx, ui, batch_index, None, UiLayout::Inline);
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
