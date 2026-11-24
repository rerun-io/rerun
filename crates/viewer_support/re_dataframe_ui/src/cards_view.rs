use std::num::NonZeroUsize;
use std::str::FromStr as _;

use egui::{Frame, NumExt as _, RichText, Ui};

use re_sdk_types::blueprint::components::{ColumnName, TableLayoutKind};
use re_ui::{DesignTokens, UiExt as _, UiLayout};
use re_viewer_context::{AppContext, ViewStates};

use crate::DisplayRecordBatch;
use crate::blueprint::{CardLayout, TableColumn};
use crate::datafusion_table_widget::{DataColumns, EditableColumn, find_row_batch};
use crate::display_record_batch::DisplayColumn;
use crate::preview_renderer::{PreviewLayout, SegmentPreviewRenderer};

const PREFERRED_PREVIEW_HEIGHT: f32 = 200.0;
const MAX_PREVIEW_HEIGHT: f32 = 400.0;

/// Allow at most 15% shrinkage to fit another column while keeping previews near their preferred size.
const MIN_PREVIEW_HEIGHT: f32 = 170.0;

/// Room around the card content above and below the preview.
const CARD_MARGIN: egui::Margin = egui::Margin::symmetric(16, 12);

pub struct CellChangeEvent {
    pub row: u64,
    pub physical_column: ColumnName,

    /// The edited data of the cell, as the component UI returned it.
    pub new_value: arrow::array::ArrayRef,
}

/// Shared parameters that are the same for every card in the grid.
struct CardConfig<'a> {
    title_col_index: Option<usize>,
    url_col_index: Option<usize>,
    card_layout: &'a CardLayout<'a>,
    flag_field: Option<&'a TableColumn<'a>>,
    editable_columns: &'a [EditableColumn],
    preview_layouts: &'a [PreviewLayout],
    max_preview_height: f32,
}

impl CardConfig<'_> {
    /// Whether the values of the field can be edited.
    fn is_editable(&self, field: &TableColumn<'_>) -> bool {
        self.editable_columns
            .iter()
            .any(|column| column.physical_name == *field.physical_name())
    }
}

/// Render the data using the card layout.
///
/// Returns a list of cell changes that need to be applied to the underlying data.
pub fn cards_ui(
    ctx: &AppContext<'_>,
    ui: &mut Ui,
    columns: &DataColumns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_layout: &CardLayout<'_>,
    view_renderers: &[SegmentPreviewRenderer<'_>],
    view_states: &mut ViewStates,
    num_table_rows: u64,
    editable_columns: &[EditableColumn],
    marked_rows: &[bool],
) -> Vec<CellChangeEvent> {
    let mut cell_changes = Vec::new();

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

    // Preferred ratios can change as recordings load, so the grid can reflow after rendering.
    let preview_layouts = view_renderers
        .iter()
        .map(|renderer| renderer.preferred_layout(ctx, view_states))
        .collect::<Vec<_>>();
    let card_min_width = preview_layouts
        .iter()
        .map(|layout| layout.width_for_height(MIN_PREVIEW_HEIGHT))
        .fold(tokens.table_grid_view_card_min_width, f32::max);

    let card_preferred_width = preview_layouts
        .iter()
        .map(|layout| layout.width_for_height(PREFERRED_PREVIEW_HEIGHT))
        .fold(card_min_width, f32::max);

    let card_frame = Frame::new()
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    // Parent content can be wider than the visible viewport.
    let visible_rect = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    let card_config = CardConfig {
        title_col_index,
        url_col_index,
        card_layout,
        flag_field: card_layout.flag(),
        editable_columns,
        preview_layouts: &preview_layouts,
        max_preview_height: visible_rect
            .height()
            .at_least(0.0)
            .at_most(MAX_PREVIEW_HEIGHT),
    };

    ui.scope_builder(egui::UiBuilder::new().max_rect(visible_rect), |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .content_margin(egui::Margin::same(card_spacing as i8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(card_spacing, card_spacing);

                let items = (0..num_table_rows as usize)
                    .map(|row| re_ui::egui_ext::card_layout::CardLayoutItem {
                        min_width: card_min_width + card_spacing,
                        frame: marked_rows
                            .get(row)
                            .copied()
                            .unwrap_or(false)
                            .then_some(card_frame.fill(tokens.card_marked_fill)),
                        clickable: None,
                    })
                    .collect();
                let mut grid = re_ui::egui_ext::card_layout::CardLayout::new(items, card_frame)
                    .all_rows_use_available_width(false)
                    .hover_stroke(tokens.card_hover_stroke);
                if !preview_layouts.is_empty() {
                    grid = grid.fixed_column_count(preview_card_columns(
                        ui.available_width(),
                        card_spacing,
                        card_min_width,
                        card_preferred_width,
                    ));
                }
                grid.show(ui, |ui, index, card_hovered| {
                    cell_changes.extend(card_content_ui(
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
    });

    cell_changes
}

/// Choose the column count whose full-row width is closest to the preferred width.
/// Cards grow beyond their preferred width to fill complete rows.
fn preview_card_columns(
    available: f32,
    spacing: f32,
    minimum: f32,
    preferred: f32,
) -> NonZeroUsize {
    let available = available.at_least(0.0);
    let minimum = minimum.at_most(available);
    let preferred = preferred.at_least(minimum).at_most(available);
    let max_columns = ((available + spacing) / (minimum + spacing))
        .floor()
        .at_least(1.0) as usize;

    let width_for_columns =
        |columns: usize| (available - spacing * (columns - 1) as f32) / columns as f32;

    (1..=max_columns)
        .min_by(|&a, &b| {
            (width_for_columns(a) - preferred)
                .abs()
                .total_cmp(&(width_for_columns(b) - preferred).abs())
        })
        .and_then(NonZeroUsize::new)
        .unwrap_or(NonZeroUsize::MIN)
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
///
/// Returns the edits made to the card's fields.
fn card_content_ui(
    ctx: &AppContext<'_>,
    config: &CardConfig<'_>,
    ui: &mut Ui,
    view_renderers: &[SegmentPreviewRenderer<'_>],
    view_states: &mut ViewStates,
    row_idx: u64,
    data_columns: &DataColumns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    card_hovered: bool,
) -> Vec<CellChangeEvent> {
    re_tracing::profile_function!();

    let &CardConfig {
        title_col_index,
        url_col_index,
        card_layout,
        flag_field,
        editable_columns: _,
        preview_layouts,
        max_preview_height,
    } = config;

    let Some((display_record_batch, batch_index)) =
        find_row_batch(display_record_batches, row_idx as usize)
    else {
        return Vec::new();
    };

    let mut cell_changes = Vec::new();

    // Set when the title is clicked, to whether the recording opens in a new tab.
    let mut open_recording: Option<bool> = None;

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
        ui.spacing_mut().item_spacing.y = 0.0;

        // Title row: title on the left (truncate if needed), flag on the right.
        Frame::new().inner_margin(CARD_MARGIN).show(ui, |ui| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    if let Some(title_text) = title_text {
                        let response = ui.add(
                            egui::Label::new(
                                RichText::new(title_text)
                                    .monospace()
                                    .strong()
                                    .text_style(DesignTokens::heading_4())
                                    .color(ui.tokens().text_default),
                            )
                            .truncate()
                            .sense(egui::Sense::click()),
                        );
                        if response.hovered() {
                            ui.painter().line_segment(
                                [response.rect.left_bottom(), response.rect.right_bottom()],
                                egui::Stroke::new(1.0, ui.tokens().text_default),
                            );
                        }
                        let response = response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Open recording");

                        // The usual modifiers open the recording in a new tab.
                        open_recording = if response.clicked_with_open_in_background() {
                            Some(true)
                        } else if response.clicked() {
                            Some(false)
                        } else {
                            None
                        };
                    }
                },
                |ui| {
                    if let Some(flag_field) = flag_field
                        && let Some(column_index) =
                            data_columns.index_by_physical_name(flag_field.physical_name())
                        && let Some(column) = display_record_batch.columns().get(column_index)
                    {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // The card header has room for the flag button at full size.
                            ui.spacing_mut().interact_size.y = re_ui::FLAG_BUTTON_SIZE;

                            let component_descr = flag_field.resolved_component_descriptor();
                            if let Some(new_value) = column.data_ui(
                                ctx,
                                ui,
                                batch_index,
                                None,
                                UiLayout::List,
                                component_descr.as_ref(),
                                config.is_editable(flag_field),
                            ) {
                                cell_changes.push(CellChangeEvent {
                                    row: row_idx,
                                    physical_column: flag_field.physical_name().clone(),
                                    new_value,
                                });
                            }
                        });
                    }
                },
            );
        });

        for (renderer, layout) in std::iter::zip(view_renderers, preview_layouts) {
            let preview_height = layout
                .height_for_width(ui.available_width())
                .at_most(max_preview_height);

            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), preview_height),
                egui::Sense::hover(),
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("preview", renderer.data_column_index()))
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

        // The fields shown under the preview, with the cell that renders each one.
        let fields = card_layout
            .fields()
            .iter()
            .filter(|field| field.is_visible(TableLayoutKind::Cards))
            .filter_map(|field| {
                let col_idx = data_columns.index_by_physical_name(field.physical_name())?;
                if Some(col_idx) == title_col_index {
                    return None;
                }
                let column = display_record_batch.columns().get(col_idx)?;

                // Preview fields and the header flag are shown separately.
                if field.is_preview()
                    || flag_field.is_some_and(|flag| flag.physical_name() == field.physical_name())
                {
                    return None;
                }
                let component_descr =
                    field.value_resolved_component_descriptor(column, batch_index);

                Some((field, column, component_descr))
            })
            .collect::<Vec<_>>();

        if fields.is_empty() {
            // The card ends with its bottom margin under the preview.
            ui.add_space(CARD_MARGIN.bottom as f32);
        } else {
            Frame::new().inner_margin(CARD_MARGIN).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(14.0, 8.0);
                    ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.5));

                    for (field, column, component_descr) in fields {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(RichText::new(field.display_name()).weak());
                        ui.spacing_mut().item_spacing.x = 14.0;
                        if let Some(new_value) = column.data_ui(
                            ctx,
                            ui,
                            batch_index,
                            None,
                            UiLayout::Inline,
                            component_descr.as_ref(),
                            config.is_editable(field),
                        ) {
                            cell_changes.push(CellChangeEvent {
                                row: row_idx,
                                physical_column: field.physical_name().clone(),
                                new_value,
                            });
                        }
                    }
                });
            });
        }
    });

    if let Some(new_tab) = open_recording
        && let Some(idx) = url_col_index
        && let Some(DisplayColumn::Component(comp)) = display_record_batch.columns().get(idx)
        && let Some(value) = comp.string_value_at(batch_index)
        && let Ok(uri) = match ctx.route.origin() {
            Some(base) => re_uri::RedapUri::parse_with_base(base, &value),
            None => re_uri::RedapUri::from_str(&value),
        }
    {
        ui.open_url(egui::OpenUrl {
            url: uri.to_string(),
            new_tab,
        });
    }

    cell_changes
}

#[cfg(test)]
mod tests {
    use super::preview_card_columns;

    #[test]
    fn preview_columns_balance_preferred_and_readable_widths() {
        for (available, minimum, preferred, expected) in [
            (1000.0, 350.0, 350.0, 2),
            (300.0, 350.0, 350.0, 1),
            (0.0, 350.0, 350.0, 1),
            (1000.0, 350.0, 402.0, 2),
            (500.0, 350.0, 402.0, 1),
            (760.0, 350.0, 402.0, 2),
            (1100.0, 350.0, 402.0, 3),
            (300.0, 606.0, 714.0, 1),
        ] {
            assert_eq!(
                preview_card_columns(available, 8.0, minimum, preferred).get(),
                expected,
                "available width: {available}, minimum: {minimum}, preferred: {preferred}",
            );
        }
    }
}
