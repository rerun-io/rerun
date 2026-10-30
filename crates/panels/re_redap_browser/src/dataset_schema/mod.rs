use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use arrow::datatypes::Schema;
use egui::{AtomExt as _, Frame, Margin, RichText};
use egui_table::{CellInfo, HeaderCellInfo};
use re_dataframe_ui::ReTable;
use re_log_types::EntityPath;
use re_log_types::external::re_types_core::reflection::ComponentDescriptorExt as _;
use re_redap_client::ApiError;
use re_sorbet::{ColumnError, ColumnKind, ComponentColumnDescriptor};
use re_ui::filter_widget::{FilterState, PathRanges, format_matching_text};
use re_ui::list_item;
use re_ui::{ButtonExt as _, ServerValue, TableStyle, UiExt as _, icons};

#[cfg(test)]
mod tests;

/// The space between an entity's component count and the right edge of its row in the entity tree.
const COUNT_PADDING: i8 = 8;

/// The widest a column of the component table gets when fitted to its content.
///
/// Longer text is cut at the end, and data types show fewer levels of nesting.
const MAX_FITTED_COLUMN_WIDTH: f32 = 400.0;

/// The components of a dataset, grouped into an entity tree.
#[derive(Debug, Default)]
pub struct DatasetSchema {
    entities: BTreeMap<EntityPath, SchemaEntity>,

    /// Number of entities that have at least one component.
    ///
    /// The remaining entries of `entities` only group children.
    entity_count: usize,

    component_count: usize,
}

#[derive(Debug, Default)]
struct SchemaEntity {
    children: BTreeSet<EntityPath>,
    components: Vec<SchemaComponent>,
}

#[derive(Debug)]
struct SchemaComponent {
    descriptor: ComponentColumnDescriptor,

    /// The Arrow field metadata, including the keys that `descriptor` already models.
    metadata: BTreeMap<String, String>,
}

/// The visible entities, each with the highlighted ranges of its own name.
type EntityHighlights = BTreeMap<EntityPath, Vec<Range<usize>>>;

impl DatasetSchema {
    pub fn from_arrow(schema: &Schema) -> Result<Self, ColumnError> {
        let mut result = Self::default();

        for field in schema.fields() {
            match ColumnKind::try_from(field.as_ref())? {
                ColumnKind::RowId | ColumnKind::Index => {}
                ColumnKind::Component => {
                    let descriptor = ComponentColumnDescriptor::from_arrow_field(None, field)?;
                    let path = descriptor.entity_path.clone();
                    let metadata = field
                        .metadata()
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect();

                    result
                        .entities
                        .entry(path.clone())
                        .or_default()
                        .components
                        .push(SchemaComponent {
                            descriptor,
                            metadata,
                        });
                    result.component_count += 1;
                    result.insert_parents(path);
                }
            }
        }

        for entity in result.entities.values_mut() {
            entity
                .components
                .sort_by(|a, b| a.descriptor.cmp(&b.descriptor));
        }

        result.entity_count = result
            .entities
            .values()
            .filter(|entity| !entity.components.is_empty())
            .count();

        Ok(result)
    }

    /// Adds every ancestor of `path` to the tree, each listing the next one down as a child.
    fn insert_parents(&mut self, mut path: EntityPath) {
        while let Some(parent) = path.parent() {
            self.entities
                .entry(parent.clone())
                .or_default()
                .children
                .insert(path);
            path = parent;
        }
    }

    /// The entities matching `filter`, plus all their ancestors.
    fn visible_entities(&self, filter: &FilterState) -> EntityHighlights {
        let matcher = filter.filter();

        // Each entity collects the ranges of its own name at part 0.
        // `PathRanges` sorts them and merges the ones that overlap.
        let mut visible = BTreeMap::<EntityPath, PathRanges>::new();
        for path in self.entities.keys() {
            let Some(mut path_ranges) =
                matcher.match_path(path.iter().map(|part| part.unescaped_str()))
            else {
                continue;
            };

            for ancestor in std::iter::successors(Some(path.clone()), EntityPath::parent) {
                let ranges = visible.entry(ancestor.clone()).or_default();
                if let Some(part_index) = ancestor.len().checked_sub(1)
                    && let Some(part_ranges) = path_ranges.remove(part_index)
                {
                    ranges.extend(0, part_ranges);
                }
            }
        }

        visible
            .into_iter()
            .map(|(path, mut ranges)| {
                let ranges = ranges.remove(0).map(Iterator::collect).unwrap_or_default();
                (path, ranges)
            })
            .collect()
    }
}

#[derive(Clone)]
struct SchemaUiState {
    filter: FilterState,
    selected_entity: Option<EntityPath>,
    show_archetype: bool,
    show_full_names: bool,
    show_data_type: bool,
    show_index_type: bool,
    show_metadata: bool,

    /// The width of the widest entity row in the previous pass.
    entity_tree_width: f32,
}

impl Default for SchemaUiState {
    fn default() -> Self {
        Self {
            filter: FilterState::default(),
            selected_entity: None,
            entity_tree_width: 0.0,
            show_archetype: true,
            show_full_names: false,
            show_data_type: true,
            show_index_type: true,
            show_metadata: false,
        }
    }
}

pub fn schema_ui(ui: &mut egui::Ui, schema: &ServerValue<Arc<DatasetSchema>, ApiError>) {
    Frame::new()
        .inner_margin(Margin {
            left: 16,
            right: 16,
            top: 0,
            bottom: re_ui::TAB_TOOLBAR_MARGIN_Y as i8,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.visuals_mut().widgets.noninteractive.fg_stroke.color = ui.tokens().text_default;

            if let Some(err) = schema.get_err() {
                ui.error_label(format!("Failed to fetch dataset schema: {err}"));
            }

            let Some(schema) = schema.get() else {
                if schema.get_err().is_none() {
                    ui.add_space(16.0);
                    ui.loading_indicator("Loading schema…");
                }
                return;
            };

            let id = ui.make_persistent_id("schema_state");
            let mut state =
                ui.data_mut(|data| data.get_temp::<SchemaUiState>(id).unwrap_or_default());
            state.ui(ui, schema);
            ui.data_mut(|data| data.insert_temp(id, state));
        });
}

impl SchemaUiState {
    fn ui(&mut self, ui: &mut egui::Ui, schema: &DatasetSchema) {
        // The toolbar frame's bottom margin is the whole gap down to the card.
        let item_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing.y = 0.0;

        self.toolbar_ui(ui, schema);

        if schema.entities.is_empty() {
            ui.add_space(32.0);
            ui.vertical_centered(|ui| {
                ui.weak("This dataset has no components");
            });
            return;
        }

        if self
            .selected_entity
            .as_ref()
            .is_some_and(|path| !schema.entities.contains_key(path))
        {
            self.selected_entity = None;
        }

        let tokens = ui.tokens();
        Frame::new()
            .fill(tokens.card_fill)
            .stroke(tokens.card_stroke)
            .corner_radius(tokens.table_grid_view_card_corner_radius)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = item_spacing;
                ui.set_min_size(ui.available_size());

                egui::Panel::left(ui.scope_id().with("schema_entities"))
                    .default_size(300.0)
                    .size_range(160.0..=450.0)
                    .resizable(true)
                    .frame(Frame::new().inner_margin(12))
                    .show(ui, |ui| self.entity_tree_ui(ui, schema));

                egui::CentralPanel::default()
                    .frame(Frame::new().inner_margin(16))
                    .show(ui, |ui| self.components_ui(ui, schema));
            });
    }

    /// The entity and component counts, and the display options.
    fn toolbar_ui(&mut self, ui: &mut egui::Ui, schema: &DatasetSchema) {
        Frame::new()
            .inner_margin(Margin::symmetric(0, re_ui::TAB_TOOLBAR_MARGIN_Y as i8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(re_ui::TAB_TOOLBAR_HEIGHT);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let popup_id = ui.make_persistent_id("display_menu");
                        let response = ui.add(
                            egui::Button::new((icons::SETTINGS, "Display"))
                                .outlined()
                                .size_small()
                                .highlighted(egui::Popup::is_id_open(ui.ctx(), popup_id)),
                        );

                        egui::Popup::menu(&response)
                            .id(popup_id)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.checkbox(&mut self.show_archetype, "Archetype");
                                ui.checkbox(&mut self.show_full_names, "Full component names");
                                ui.checkbox(&mut self.show_data_type, "Data type");
                                ui.checkbox(&mut self.show_index_type, "Index type");
                                ui.checkbox(&mut self.show_metadata, "Metadata");
                            });

                        let entities = if schema.entity_count == 1 {
                            "entity"
                        } else {
                            "entities"
                        };
                        ui.label(format!(
                            "{} {entities} · {}",
                            re_format::format_uint(schema.entity_count),
                            re_format::format_plural_s(schema.component_count, "component"),
                        ));
                    });
                });
            });
    }

    /// The entity tree with its filter field.
    fn entity_tree_ui(&mut self, ui: &mut egui::Ui, schema: &DatasetSchema) {
        let tokens = ui.tokens();
        ui.scope(|ui| {
            let widgets = &mut ui.visuals_mut().widgets;
            for visuals in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                visuals.corner_radius = 8.into();
            }
            ui.visuals_mut().selection.stroke = tokens.text_edit_outlined.stroke_focused;

            self.filter.search_field_ui(ui, "Filter entity paths…");
        });

        let visible = schema.visible_entities(&self.filter);

        egui::ScrollArea::both()
            .id_salt("entity_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(self.entity_tree_width);

                let rows_right = Cell::new(ui.max_rect().left());
                ui.full_span_scope(ui.max_rect().x_range(), |ui| {
                    list_item::list_item_scope(ui, "entities", |ui| {
                        if visible.is_empty() {
                            ui.label("No matching entities");
                        } else {
                            self.entity_ui(ui, schema, &EntityPath::root(), &visible, &rows_right);
                        }
                    });
                });

                // The rows only know their width once laid out, so a pass that measured a
                // different width than it used is discarded.
                let width = (rows_right.get() - ui.max_rect().left()).ceil();
                if width != self.entity_tree_width {
                    self.entity_tree_width = width;
                    ui.ctx().request_discard("Schema entity tree changed width");
                }
            });
    }

    fn entity_ui(
        &mut self,
        ui: &mut egui::Ui,
        schema: &DatasetSchema,
        path: &EntityPath,
        visible: &EntityHighlights,
        rows_right: &Cell<f32>,
    ) {
        let (Some(highlights), Some(entity)) = (visible.get(path), schema.entities.get(path))
        else {
            return;
        };

        let name = entity_name(path);
        let component_count = entity.components.len();

        let label = format_matching_text(
            ui.ctx(),
            &name,
            highlights.iter().cloned(),
            &egui::TextStyle::Monospace,
            None,
        );

        let content = list_item::CustomContent::new(move |ui, context| {
            context.response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::Button,
                    ui.is_enabled(),
                    context.list_item.selected,
                    &name,
                )
            });

            let count = re_format::format_uint(component_count);
            let mut atoms = egui::Atoms::new(label.atom_shrink(true));
            if component_count > 0 {
                atoms.push_right(egui::Atom::grow());
                atoms.push_right(RichText::new(&count).small());
            }

            // The row ends at the visible right edge of the tree, so the component count stays in
            // view while the tree is scrolled horizontally. A name that does not fit is shortened.
            let mut rect = ui.max_rect();
            rect.max.x = rect.right().min(ui.clip_rect().right());

            let response = ui.put(
                rect,
                egui::WidgetAtom::new(atoms)
                    .gap(ui.tokens().text_to_icon_padding())
                    .frame(Frame::new().inner_margin(Margin {
                        right: COUNT_PADDING,
                        ..Margin::ZERO
                    }))
                    .align2(egui::Align2::LEFT_CENTER)
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .fallback_text_color(context.visuals.text_color()),
            );

            if let Some(size) = response.intrinsic_size() {
                rows_right.set(rows_right.get().max(response.rect.left() + size.x));
            }

            if component_count > 0 {
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &count));
            }
        });

        let item = ui
            .list_item()
            .with_corner_radius(7)
            .selected(self.selected_entity.as_ref() == Some(path));

        let mut response = if entity.children.is_empty() {
            item.show_hierarchical(ui, content)
        } else {
            item.collapse_temporary(self.filter.is_active())
                .show_hierarchical_with_children(
                    ui,
                    ui.make_persistent_id((path, self.filter.session_id())),
                    true,
                    content,
                    |ui| {
                        for child in &entity.children {
                            self.entity_ui(ui, schema, child, visible, rows_right);
                        }
                    },
                )
                .item_response
        };

        // The label shows only the last part of the path, so nested entities get the full path on hover.
        if path.len() > 1 {
            response = response.on_hover_text(path.to_string());
        }

        if response.clicked()
            || list_item::ListItem::gained_focus_via_arrow_key(ui.ctx(), response.id)
        {
            self.selected_entity = Some(path.clone());
        }
    }

    fn components_ui(&self, ui: &mut egui::Ui, schema: &DatasetSchema) {
        let Some((path, entity)) = self
            .selected_entity
            .as_ref()
            .and_then(|path| schema.entities.get_key_value(path))
        else {
            ui.add_space(32.0);
            ui.vertical_centered(|ui| {
                ui.weak("Select an entity to see its components");
            });
            return;
        };

        ui.horizontal(|ui| entity_heading_ui(ui, path, entity.components.len()));
        ui.add_space(8.0);

        if entity.components.is_empty() {
            ui.separator();
            ui.label("No components logged at this entity. It only groups children.");
            return;
        }

        let columns = self.visible_columns();

        // Each entity and set of columns gets its own table, which fits its columns to their
        // content when first shown.
        let session_id =
            ui.make_persistent_id(("components", path, &columns, self.show_full_names));

        let mut table = ComponentTable {
            state: self,
            components: &entity.components,
            columns: &columns,
            row_height: ui.tokens().table_row_height(TableStyle::Spacious),
        };

        ReTable::new(
            ui.ctx(),
            session_id,
            &mut table,
            columns
                .iter()
                .enumerate()
                .map(|(index, column)| (session_id.with(column), index)),
            entity.components.len() as u64,
        )
        .with_row_numbers(false)
        .show(ui);
    }

    fn visible_columns(&self) -> Vec<ComponentColumn> {
        [
            (ComponentColumn::Archetype, self.show_archetype),
            (ComponentColumn::Component, true),
            (ComponentColumn::DataType, self.show_data_type),
            (ComponentColumn::IndexType, self.show_index_type),
            (ComponentColumn::Metadata, self.show_metadata),
        ]
        .into_iter()
        .filter_map(|(column, visible)| visible.then_some(column))
        .collect()
    }

    fn component_cell_ui(
        &self,
        ui: &mut egui::Ui,
        component: &SchemaComponent,
        column: ComponentColumn,
    ) {
        let descriptor = &component.descriptor;
        match column {
            ComponentColumn::Archetype => {
                let archetype = descriptor.archetype;
                cell_label(
                    ui,
                    archetype.map_or("", |name| name.short_name()),
                    archetype.map(|archetype| archetype.full_name()),
                );
            }
            ComponentColumn::Component => {
                let display_name = descriptor.display_name();
                let name = if self.show_full_names {
                    display_name.to_owned()
                } else {
                    short_component_name(descriptor)
                };

                cell_label(ui, &name, Some(display_name));
            }
            ComponentColumn::DataType => {
                data_type_cell_ui(ui, &descriptor.store_datatype);
            }
            ComponentColumn::IndexType => {
                cell_label(
                    ui,
                    if descriptor.is_static {
                        "Static"
                    } else {
                        "Temporal"
                    },
                    None,
                );
            }
            ComponentColumn::Metadata => {
                metadata_cell_ui(ui, &component.metadata);
            }
        }
    }
}

/// A column of the component table, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ComponentColumn {
    Archetype,
    Component,
    DataType,
    IndexType,
    Metadata,
}

impl ComponentColumn {
    fn header(self) -> &'static str {
        match self {
            Self::Archetype => "ARCHETYPE",
            Self::Component => "COMPONENT",
            Self::DataType => "DATA TYPE",
            Self::IndexType => "INDEX TYPE",
            Self::Metadata => "METADATA",
        }
    }
}

/// The components of the selected entity, one row each.
struct ComponentTable<'a> {
    state: &'a SchemaUiState,
    components: &'a [SchemaComponent],
    columns: &'a [ComponentColumn],
    row_height: f32,
}

impl egui_table::TableDelegate for ComponentTable<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        if let Some(column) = self.columns.get(cell.group_index) {
            ui.label(RichText::new(column.header()).monospace().size(10.0));
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        if let Some(component) = self.components.get(cell.row_nr as usize)
            && let Some(column) = self.columns.get(cell.col_nr)
        {
            self.state.component_cell_ui(ui, component, *column);
        }
    }

    fn default_row_height(&self) -> f32 {
        self.row_height
    }
}

/// The entity path, shortened from the start to fit, followed by the component count.
fn entity_heading_ui(ui: &mut egui::Ui, path: &EntityPath, component_count: usize) {
    let count = egui::WidgetText::from(re_format::format_plural_s(component_count, "component"))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
    let path_width = (ui.available_width() - count.size().x - ui.spacing().item_spacing.x).max(0.0);

    let full_path = path.to_string();
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let label = re_ui::egui_ext::shorten_from_start(ui, &full_path, &font_id, path_width);

    ui.add(egui::Label::new(RichText::new(label).monospace().weak()).extend())
        .on_hover_text(full_path);
    ui.label(count);
}

fn metadata_cell_ui(ui: &mut egui::Ui, metadata: &BTreeMap<String, String>) {
    if metadata.is_empty() {
        cell_label(ui, "None", None);
        return;
    }

    let response = ui
        .add(egui::Button::dropdown(re_format::format_plural_s(metadata.len(), "key")).size_tiny());

    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| metadata_snippet_ui(ui, metadata));
}

/// The Arrow field metadata as a code snippet with a copy button.
fn metadata_snippet_ui(ui: &mut egui::Ui, metadata: &BTreeMap<String, String>) {
    let tokens = ui.tokens();
    let text = metadata
        .iter()
        .map(|(key, value)| format!("{key} = {value}"))
        .collect::<Vec<_>>()
        .join("\n");

    let frame = Frame::new()
        .fill(tokens.table_header_bg_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(6)
        .inner_margin(Margin {
            left: 10,

            // Leaves the top right corner free for the copy button.
            right: 34,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            // Solid scroll bars take their own space next to the text.
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();

            egui::ScrollArea::both()
                .max_width(520.0)
                .max_height(240.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&text).monospace().color(tokens.text_default),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .selectable(true),
                    );
                });
        });

    if ui.rect_contains_pointer(frame.response.rect) {
        let button = ui.small_icon_button_widget(&icons::COPY, "Copy metadata");
        let size = egui::Vec2::splat(tokens.small_icon_size.y + 8.0);
        let corner = frame.response.rect.right_top() + egui::vec2(-4.0, 4.0);
        let rect = egui::Rect::from_min_size(corner - egui::vec2(size.x, 0.0), size);

        if ui.put(rect, button).clicked() {
            ui.copy_text(text);
        }
    }
}

fn entity_name(path: &EntityPath) -> String {
    path.last()
        .map_or_else(|| "/ (root)".to_owned(), |part| part.ui_string())
}

fn short_component_name(descriptor: &ComponentColumnDescriptor) -> String {
    descriptor
        .component_descriptor()
        .archetype_field_name()
        .to_owned()
}

/// The width a cell has for its text, next to the space for its copy button.
///
/// When the table fits a column to its content, the cell is at most [`MAX_FITTED_COLUMN_WIDTH`] wide.
fn cell_text_width(ui: &egui::Ui) -> f32 {
    let cell_width = if ui.is_sizing_pass() {
        MAX_FITTED_COLUMN_WIDTH
    } else {
        ui.available_width()
    };
    (cell_width - copy_button_space(ui)).max(0.0)
}

/// The space a cell keeps at its right edge for the copy button it shows on hover.
fn copy_button_space(ui: &egui::Ui) -> f32 {
    re_ui::Size::Tiny.icon_button_size().x + ui.spacing().item_spacing.x
}

/// The data type with as many levels of nesting as fit the cell, and the full data type on hover.
fn data_type_cell_ui(ui: &mut egui::Ui, data_type: &arrow::datatypes::DataType) {
    let full = re_arrow_ui::data_type_syntax_highlighted(data_type, ui.style());
    let full_galley = truncated_galley(ui, full.clone());
    if !full_galley.elided {
        cell_galley_label(ui, full_galley, full);
        return;
    }

    // A deeper depth is never narrower, so a binary search finds the deepest depth that fits.
    // `fitting` fits or is 0, and `too_wide` does not fit.
    let galley_at_depth = |depth| {
        let job = re_arrow_ui::data_type_syntax_highlighted_to_depth(data_type, depth, ui.style());
        truncated_galley(ui, job)
    };
    let mut fitting = 0;
    let mut fitting_galley = None;
    let mut too_wide = re_arrow_ui::data_type_nesting_depth(data_type);
    while too_wide - fitting > 1 {
        let depth = fitting + (too_wide - fitting) / 2;
        let galley = galley_at_depth(depth);
        if galley.elided {
            too_wide = depth;
        } else {
            fitting = depth;
            fitting_galley = Some(galley);
        }
    }

    let shown = fitting_galley.unwrap_or_else(|| galley_at_depth(fitting));
    cell_galley_label(ui, shown, full);
}

/// Monospace `text` on one line, cut at the end to fit the cell.
///
/// `full_text` is the complete form of `text`, like the full name of an archetype.
/// See [`cell_galley_label`].
fn cell_label(ui: &mut egui::Ui, text: &str, full_text: Option<&str>) {
    let monospace_job = |text: &str| {
        let job = egui::WidgetText::from(RichText::new(text).monospace()).into_layout_job(
            ui.style(),
            egui::FontSelection::Default,
            egui::Align::Center,
        );
        Arc::unwrap_or_clone(job)
    };
    let job = monospace_job(text);
    let full = full_text.map_or_else(|| job.clone(), monospace_job);
    let galley = truncated_galley(ui, job);
    cell_galley_label(ui, galley, full);
}

/// `job` laid out on one line, cut at the end to fit the cell.
fn truncated_galley(ui: &egui::Ui, mut job: egui::text::LayoutJob) -> Arc<egui::Galley> {
    job.wrap = egui::text::TextWrapping::truncate_at_width(cell_text_width(ui));
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

/// `galley` as the content of a cell.
///
/// `full` is the complete form of the galley's text. When the cell shows less than that, the
/// full text shows on hover and the label is not selectable.
/// Hovering the cell shows a button at its right edge that copies the full text.
fn cell_galley_label(ui: &mut egui::Ui, galley: Arc<egui::Galley>, full: egui::text::LayoutJob) {
    let cell_rect = ui.max_rect();
    let is_complete = !galley.elided && galley.text() == full.text;

    let response = ui.add(egui::Label::new(galley).extend().selectable(is_complete));

    if ui.is_sizing_pass() {
        ui.add_space(copy_button_space(ui));
    } else if ui.rect_contains_pointer(cell_rect) {
        let size = re_ui::Size::Tiny.icon_button_size();
        let rect = egui::Rect::from_center_size(
            egui::pos2(cell_rect.right() - size.x / 2.0, cell_rect.center().y),
            size,
        );
        if ui
            .put(
                rect,
                egui::Button::icon(icons::COPY, "Copy").size_tiny().ghost(),
            )
            .clicked()
        {
            re_log::info!("Copied {:?}", full.text);
            ui.copy_text(full.text.clone());
        }
    }

    if !is_complete {
        response.on_hover_ui(|ui| {
            ui.label(full);
        });
    }
}
