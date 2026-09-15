use egui::containers::menu::{MenuButton, MenuConfig};
use egui::emath::GuiRounding as _;
use egui::{Color32, Frame, Id, Label, Link, PopupCloseBehavior, RichText, Stroke, Style};
use re_sdk_types::blueprint::components::{ColumnName, TableLayoutKind};
use re_ui::text_edit::{ReTextEdit, TextEditVariant};
use re_ui::{UiExt as _, design_tokens_of, icons};
use re_viewer_context::AppBlueprintCtx;

use crate::blueprint::TableBlueprint;
use crate::blueprint::TableColumn;

pub const CELL_SEPARATOR_STROKE_OFFSET: f32 = 0.5;

/// This applies some fixes so that the column resize bar is correctly displayed.
///
/// Remember to revert the styling within the cells!
pub fn apply_table_style_fixes(style: &mut Style) {
    let theme = if style.visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };

    let design_tokens = design_tokens_of(theme);

    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0, design_tokens.table_interaction_hovered_bg_stroke);
    style.visuals.widgets.active.bg_stroke =
        Stroke::new(1.0, design_tokens.table_interaction_active_bg_stroke);
    // regular vertical lines are drawn in cell_ui to allow cells to be connected
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
}

pub fn header_ui<R>(
    ui: &mut egui::Ui,
    table_style: re_ui::TableStyle,
    connected_to_next_cell: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let rect = ui
        .max_rect()
        .round_to_pixels(ui.pixels_per_point())
        .round_ui();

    ui.painter()
        .rect_filled(rect, 0.0, ui.tokens().table_header_bg_fill);

    let response = Frame::new()
        .inner_margin(ui.tokens().header_cell_margin(table_style))
        .show(ui, content);

    if !connected_to_next_cell {
        ui.painter().vline(
            rect.max.x - CELL_SEPARATOR_STROKE_OFFSET,
            rect.y_range(),
            Stroke::new(1.0, ui.tokens().table_header_stroke_color),
        );
    }

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - CELL_SEPARATOR_STROKE_OFFSET, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, ui.tokens().table_header_stroke_color),
    );

    response
}

pub fn cell_ui<R>(
    ui: &mut egui::Ui,
    table_style: re_ui::TableStyle,
    connected_to_next_cell: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let response = Frame::new()
        .inner_margin(ui.tokens().table_cell_margin(table_style))
        .show(ui, content);

    let rect = ui
        .max_rect()
        .round_to_pixels(ui.pixels_per_point())
        .round_ui();

    if !connected_to_next_cell {
        ui.painter().vline(
            rect.max.x - CELL_SEPARATOR_STROKE_OFFSET,
            rect.y_range(),
            Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
        );
    }

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - CELL_SEPARATOR_STROKE_OFFSET, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
    );

    response
}

struct UiColumnConfig<'a> {
    column: &'a TableColumn<'a>,
    physical_name: ColumnName,
    visible: bool,
}

struct UiTableConfig<'a> {
    columns: Vec<UiColumnConfig<'a>>,
    filter: String,
}

impl UiTableConfig<'_> {
    /// Move all visible columns before the hidden ones, keeping their relative order.
    fn sort_visible_first(&mut self) {
        self.columns.sort_by_key(|column| !column.visible);
    }

    /// At which index will the "Hidden columns" header be shown?
    ///
    /// The header goes right before the first hidden column, or last if every column is shown.
    fn hidden_header_index(&self) -> HiddenHeaderIndex {
        HiddenHeaderIndex(
            self.columns
                .iter()
                .position(|column| !column.visible)
                .unwrap_or(self.columns.len()),
        )
    }

    /// Apply a finished drag, with `from` and `to` being entry indices of the column list.
    ///
    /// The header item can't be dragged, so a drag always moves a single column.
    /// That dragged column can change visibility, if it is dragged above or below the "hidden"
    /// header.
    fn apply_entry_drag(
        &mut self,
        hidden_header_index: HiddenHeaderIndex,
        from: EntryIndex,
        to: EntryIndex,
    ) {
        let Some(column_from) = hidden_header_index.column_index(from) else {
            return;
        };
        let column_to = hidden_header_index.columns_before(to);

        if column_from.0 >= self.columns.len() || column_to.0 > self.columns.len() {
            // Out of range for `shift_vec`, which would panic.
            return;
        }

        // Set the visibility before shifting via the `from` index
        self.columns[column_from.0].visible = to <= hidden_header_index.entry();

        egui_dnd::utils::shift_vec(column_from.0, column_to.0, &mut self.columns);
    }

    fn save(&self, blueprint_ctx: &AppBlueprintCtx<'_>, layout_kind: TableLayoutKind) {
        // Saving order preserves the previous frame's visibility, so explicit changes
        // must be written afterwards.
        TableBlueprint::save_column_order(
            blueprint_ctx,
            layout_kind,
            self.columns.iter().map(|column| column.column),
        );
        for column in &self.columns {
            if column.visible != column.column.is_visible(layout_kind) {
                TableColumn::save_visibility(
                    blueprint_ctx,
                    &column.physical_name,
                    layout_kind,
                    column.visible,
                );
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, layout_kind: TableLayoutKind) {
        ui.add(
            ReTextEdit::singleline(&mut self.filter)
                .prefix(icons::SEARCH)
                .variant(TextEditVariant::Outlined)
                .hint_text("Column name"),
        );
        let filter = self.filter.trim().to_lowercase();

        let entries: Vec<(String, bool)> = self
            .columns
            .iter()
            .map(|column_config| {
                let display_name = column_config.column.display_name();
                let listed = filter.is_empty()
                    || display_name.to_lowercase().contains(&filter)
                    || column_config
                        .physical_name
                        .as_str()
                        .to_lowercase()
                        .contains(&filter);
                (display_name, listed)
            })
            .collect();

        let hidden_header_index = self.hidden_header_index();

        // The visible columns come first, so the two sections are the two halves of `entries`.
        let (shown, hidden) = entries.split_at(hidden_header_index.shown_count());
        let listed_shown_count = shown.iter().filter(|(_, listed)| *listed).count();
        let listed_hidden_count = hidden.iter().filter(|(_, listed)| *listed).count();

        let layout_name = match layout_kind {
            TableLayoutKind::Table => "table",
            TableLayoutKind::Cards => "cards",
        };
        let mut set_all_visible = None;
        let mut toggled_column = None;

        let dnd_response = egui::ScrollArea::vertical()
            .min_scrolled_height(400.0)
            .show(ui, |ui| {
                // The "shown" header is not part of dnd, since it doesn't make sense to move
                // something above it.
                if section_header_ui(
                    ui,
                    &format!("Shown in {layout_name} ({listed_shown_count})"),
                    "Hide all",
                    listed_shown_count,
                ) {
                    set_all_visible = Some(false);
                }

                egui_dnd::dnd(ui, "columns").show_custom(|ui, iter| {
                    // 1 extra entry for the "hidden" header
                    let entry_count = self.columns.len() + 1;

                    for entry_index in (0..entry_count).map(EntryIndex) {
                        if let Some(column_index) = hidden_header_index.column_index(entry_index) {
                            let (display_name, listed) = &entries[column_index.0];
                            if !listed {
                                continue;
                            }

                            if column_row_ui(
                                ui,
                                iter,
                                entry_index,
                                &self.columns[column_index.0],
                                display_name,
                            ) {
                                toggled_column = Some(column_index);
                            }
                        } else {
                            // The "hidden" header is part of dnd, so a column can be dropped
                            // above or below it.
                            ui.add_space(8.0);
                            let id = Id::new("hidden_column_section");
                            iter.next(ui, id, entry_index.0, true, |ui, item| {
                                item.ui(ui, |ui, _handle, _state| {
                                    if section_header_ui(
                                        ui,
                                        &format!("Hidden in {layout_name} ({listed_hidden_count})"),
                                        "Show all",
                                        listed_hidden_count,
                                    ) {
                                        set_all_visible = Some(true);
                                    }
                                })
                            });
                        }
                    }
                })
            })
            .inner;

        if let Some(visible) = set_all_visible {
            for (column, (_, listed)) in std::iter::zip(&mut self.columns, &entries) {
                if *listed {
                    column.visible = visible;
                }
            }
        }

        if let Some(column_index) = toggled_column {
            let column = &mut self.columns[column_index.0];
            column.visible = !column.visible;
        }

        if let Some(update) = dnd_response.final_update() {
            self.apply_entry_drag(
                hidden_header_index,
                EntryIndex(update.from),
                EntryIndex(update.to),
            );
        }

        self.sort_visible_first();
    }
}

pub fn columns_edit_menu_ui<'a>(
    ui: &mut egui::Ui,
    blueprint_ctx: &AppBlueprintCtx<'_>,
    layout_kind: TableLayoutKind,
    columns: impl Iterator<Item = &'a TableColumn<'a>>,
) {
    MenuButton::from_button(icons::TABLE_COLUMNS.as_button_with_label(ui.tokens(), "Columns"))
        .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
        .ui(ui, |ui| {
            let filter_id = ui.id().with(("column_filter", layout_kind));
            let mut config = UiTableConfig {
                columns: columns
                    .map(|column| UiColumnConfig {
                        column,
                        physical_name: column.physical_name().clone(),
                        visible: column.is_visible(layout_kind),
                    })
                    .collect(),
                filter: ui.data_mut(|data| data.get_temp::<String>(filter_id).unwrap_or_default()),
            };

            // Ensure visible columns are sorted first. This is necessary for the ui to be displayed
            // correctly. Sorting here won't affect the blueprint, as it's only changed if something
            // was dragged / toggled.
            config.sort_visible_first();

            let before: Vec<_> = config
                .columns
                .iter()
                .map(|column| (column.physical_name.clone(), column.visible))
                .collect();
            config.ui(ui, layout_kind);
            ui.data_mut(|data| data.insert_temp(filter_id, config.filter.clone()));
            let changed = !config
                .columns
                .iter()
                .map(|column| (&column.physical_name, column.visible))
                .eq(before.iter().map(|(name, visible)| (name, *visible)));
            if changed {
                config.save(blueprint_ctx, layout_kind);
            }
        });
}

/// The position of an item in the drag-and-drop list.
///
/// The list has one item per column, plus the "Hidden columns" header, so it is one longer than
/// [`UiTableConfig::columns`]. Use [`HiddenHeaderIndex`] to convert to a [`ColumnIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EntryIndex(usize);

/// The position of a column in [`UiTableConfig::columns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ColumnIndex(usize);

/// The index that splits between the visible and hidden columns.
///
/// This helper exists to deal with the index offset introduced by the "Hidden columns" header item.
/// It's a separate item within the list so that you can drag items both above (making them visible)
/// or below (hiding them).
/// The first hidden entry comes right after [`Self::entry`].
#[derive(Debug, Clone, Copy)]
struct HiddenHeaderIndex(usize);

impl HiddenHeaderIndex {
    /// Where the "hidden" header sits in the drag-and-drop list.
    fn entry(self) -> EntryIndex {
        EntryIndex(self.0)
    }

    /// How many columns are listed above the header, i.e. where [`UiTableConfig::columns`] splits.
    fn shown_count(self) -> usize {
        self.0
    }

    /// The column an entry shows, or `None` for the "hidden" header.
    ///
    /// Helper to deal with the offset introduced by the "hidden" header.
    fn column_index(self, entry_index: EntryIndex) -> Option<ColumnIndex> {
        match entry_index.cmp(&self.entry()) {
            std::cmp::Ordering::Less => Some(ColumnIndex(entry_index.0)),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(ColumnIndex(entry_index.0 - 1)),
        }
    }

    /// How many columns are listed before `entry_index`.
    fn columns_before(self, entry_index: EntryIndex) -> ColumnIndex {
        ColumnIndex(entry_index.0 - usize::from(entry_index > self.entry()))
    }
}

/// Header of a column section, with a link to show or hide all its listed columns.
///
/// The link is disabled when the section lists no columns.
///
/// Returns `true` if the link was clicked.
fn section_header_ui(ui: &mut egui::Ui, title: &str, action_label: &str, count: usize) -> bool {
    egui::Sides::new()
        .shrink_left()
        .show(
            ui,
            |ui| {
                ui.add(Label::new(RichText::new(title).strong()));
            },
            |ui| {
                ui.add_enabled(
                    count > 0,
                    Link::new(RichText::new(action_label).color(ui.tokens().selection_bg_fill)),
                )
                .clicked()
            },
        )
        .1
}

/// A single draggable column row: drag handle, column name, and a button to show or hide it.
///
/// Returns `true` if the show/hide button was clicked.
fn column_row_ui(
    ui: &mut egui::Ui,
    iter: &mut egui_dnd::ItemIterator<'_>,
    entry_index: EntryIndex,
    column: &UiColumnConfig<'_>,
    label: &str,
) -> bool {
    // The physical name identifies the column; the label is only what the user reads.
    let id = Id::new(("column", &column.physical_name));

    let mut clicked = false;

    iter.next(ui, id, entry_index.0, true, |ui, item| {
        item.ui(ui, |ui, handle, _state| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    handle.ui(ui, |ui| {
                        ui.small_icon(&icons::DND_HANDLE, Some(ui.visuals().text_color()));
                    });

                    let label = RichText::new(label);
                    ui.label(if column.visible {
                        label.strong()
                    } else {
                        label.weak()
                    });
                },
                |ui| {
                    let (icon, alt_text) = if column.visible {
                        (&icons::VISIBLE, "Hide column")
                    } else {
                        (&icons::INVISIBLE, "Show column")
                    };
                    clicked = ui
                        .small_icon_button(icon, format!("{alt_text} {label}"))
                        .clicked();
                },
            );
        })
    });

    clicked
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};
    use re_sdk_types::blueprint::archetypes;
    use re_test_context::TestContext;
    use re_viewer_context::BlueprintContext as _;

    use super::*;

    const COLUMNS: [(&str, &str, bool); 5] = [
        ("camera_front", "Front camera", true),
        ("duration", "Duration", true),
        ("camera_rear", "Rear camera", true),
        ("camera_depth", "Depth image", false),
        ("notes", "Notes", false),
    ];

    struct MenuState {
        context: TestContext,
        columns: Vec<(String, bool)>,
    }

    fn harness() -> Harness<'static, MenuState> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 340.0))
            .build_ui_state(
                |ui, state: &mut MenuState| {
                    state.context.run(&ui.ctx().clone(), |ctx| {
                        let blueprint = AppBlueprintCtx {
                            command_sender: ctx.command_sender(),
                            current_blueprint: ctx.blueprint_db(),
                            default_blueprint: None,
                            blueprint_query: ctx.blueprint_query.clone(),
                        };
                        let results = blueprint.latest_at_in_current_blueprint(
                            &"/table/layouts/table".into(),
                            [archetypes::TableLayout::descriptor_column_order().component],
                        );
                        let order = results
                            .component_batch::<ColumnName>(
                                archetypes::TableLayout::descriptor_column_order().component,
                            )
                            .unwrap_or_default();
                        let mut columns: Vec<_> = COLUMNS
                            .iter()
                            .map(|&(name, label, visible)| {
                                TableColumn::load(name.into(), &blueprint, TableLayoutKind::Table)
                                    .with_default_display_name(label)
                                    .with_default_visibility(visible)
                            })
                            .collect();
                        columns.sort_by_key(|column| {
                            order
                                .iter()
                                .position(|name| name == column.physical_name())
                                .unwrap_or(usize::MAX)
                        });
                        state.columns = columns
                            .iter()
                            .map(|column| {
                                (
                                    column.physical_name().as_str().to_owned(),
                                    column.is_visible(TableLayoutKind::Table),
                                )
                            })
                            .collect();
                        columns_edit_menu_ui(
                            ui,
                            &blueprint,
                            TableLayoutKind::Table,
                            columns.iter(),
                        );
                    });
                    state.context.handle_system_commands(ui.ctx());
                },
                MenuState {
                    context: TestContext::default(),
                    columns: Vec::new(),
                },
            );
        harness.get_by_label("Columns").click();
        harness.run();
        harness
    }

    fn filter(harness: &mut Harness<'_, MenuState>) {
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text(" CAMERA ");
        harness.run();
        assert!(harness.query_by_label("Duration").is_none());
        assert!(harness.query_by_label("Notes").is_none());
        harness.get_by_label("Front camera");
        harness.get_by_label("Rear camera");
        harness.get_by_label("Depth image");
    }

    #[track_caller]
    fn assert_columns(harness: &Harness<'_, MenuState>, expected: &[(&str, bool)]) {
        let actual: Vec<_> = harness
            .state()
            .columns
            .iter()
            .map(|(name, visible)| (name.as_str(), *visible))
            .collect();
        assert_eq!(actual, expected);
    }

    fn toggle_column(harness: &mut Harness<'_, MenuState>, label: &str, action: &str) {
        harness.get_by_label(&format!("{action} {label}")).click();
        harness.run();
    }

    fn drag_column(harness: &mut Harness<'_, MenuState>, from: &str, to: &str, below: bool) {
        let source = harness.get_by_label(from).rect();
        let target = harness.get_by_label(to).rect();
        let start = egui::pos2(source.left() - 12.0, source.center().y);
        let end = egui::pos2(start.x, if below { target.bottom() } else { target.top() });
        harness.hover_at(start);
        harness.run();
        harness.drag_at(start);
        harness.step();
        // Workaround a bug in egui_dnd, where drag only starts when dragging far enough while still
        // within the widget
        harness.hover_at(start + egui::vec2(0.0, 3.0));
        harness.step();
        harness.hover_at(end);
        harness.step();
        harness.drop_at(end);
        harness.run();
    }

    #[test]
    fn columns_menu_filtered_snapshot() {
        let mut harness = harness();
        filter(&mut harness);
        harness.get_by_label("Shown in table (2)");
        harness.get_by_label("Hidden in table (1)");
        harness.remove_cursor();
        harness.run();
        harness.snapshot("columns_menu_filtered");
    }

    #[test]
    fn columns_menu_filter_and_visibility_buttons() {
        let mut harness = harness();
        filter(&mut harness);
        toggle_column(&mut harness, "Front camera", "Hide column");
        assert_columns(
            &harness,
            &[
                ("duration", true),
                ("camera_rear", true),
                ("camera_front", false),
                ("camera_depth", false),
                ("notes", false),
            ],
        );
        toggle_column(&mut harness, "Front camera", "Show column");
        assert_columns(
            &harness,
            &[
                ("duration", true),
                ("camera_rear", true),
                ("camera_front", true),
                ("camera_depth", false),
                ("notes", false),
            ],
        );
        harness.get_by_label("Hide all").click();
        harness.run();
        harness.get_by_label("Shown in table (0)");
        assert_columns(
            &harness,
            &[
                ("duration", true),
                ("camera_rear", false),
                ("camera_front", false),
                ("camera_depth", false),
                ("notes", false),
            ],
        );
        harness.get_by_label("Show all").click();
        harness.run();
        harness.get_by_label("Hidden in table (0)");
        assert_columns(
            &harness,
            &[
                ("duration", true),
                ("camera_rear", true),
                ("camera_front", true),
                ("camera_depth", true),
                ("notes", false),
            ],
        );
    }

    #[test]
    fn columns_menu_drag_order_and_visibility() {
        let mut harness = harness();
        drag_column(&mut harness, "Rear camera", "Front camera", false);
        assert_columns(
            &harness,
            &[
                ("camera_rear", true),
                ("camera_front", true),
                ("duration", true),
                ("camera_depth", false),
                ("notes", false),
            ],
        );
        drag_column(&mut harness, "Front camera", "Depth image", true);
        assert_columns(
            &harness,
            &[
                ("camera_rear", true),
                ("duration", true),
                ("camera_depth", false),
                ("camera_front", false),
                ("notes", false),
            ],
        );
        drag_column(&mut harness, "Front camera", "Rear camera", false);
        assert_columns(
            &harness,
            &[
                ("camera_front", true),
                ("camera_rear", true),
                ("duration", true),
                ("camera_depth", false),
                ("notes", false),
            ],
        );
    }
}
