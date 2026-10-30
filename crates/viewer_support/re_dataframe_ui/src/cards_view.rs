use std::str::FromStr as _;

use egui::{Frame, RichText, Ui};

use re_sdk_types::blueprint::components::{ColumnName, TableLayoutKind};
use re_ui::{DesignTokens, UiExt as _, UiLayout};
use re_viewer_context::{AppContext, ViewStates};

use crate::DisplayRecordBatch;
use crate::blueprint::{CardLayout, TableColumn};
use crate::datafusion_table_widget::{DataColumns, EditableColumn, find_row_batch};
use crate::display_record_batch::DisplayColumn;
use crate::preview_renderer::SegmentPreviewRenderer;

/// Height of the segment preview area inside each card.
const PREVIEW_HEIGHT: f32 = 200.0;

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

    // Scale the card width with the number of views so each view keeps roughly the same
    // footprint as a single-view card.
    let max_num_views_horizontal = view_renderers
        .iter()
        .map(SegmentPreviewRenderer::num_views)
        .max()
        .unwrap_or(1);
    let card_min_width = tokens.table_grid_view_card_min_width * max_num_views_horizontal as f32;

    let card_frame = Frame::new()
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    let card_config = CardConfig {
        title_col_index,
        url_col_index,
        card_layout,
        flag_field: card_layout.flag(),
        editable_columns,
    };

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
            re_ui::egui_ext::card_layout::CardLayout::new(items, card_frame)
                .all_rows_use_available_width(false)
                .hover_stroke(tokens.card_hover_stroke)
                .show(ui, |ui, index, card_hovered| {
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

    cell_changes
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

        // TODO(RR-4510): loading indication if we're not ready to draw
        for renderer in view_renderers {
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), PREVIEW_HEIGHT),
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
