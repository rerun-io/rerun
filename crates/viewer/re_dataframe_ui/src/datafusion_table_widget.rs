use arrow::datatypes::Fields;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use egui::containers::menu::MenuConfig;
use egui::{Align, Frame, Id, Layout, Margin, RichText, Stroke, Ui, Widget as _};
use egui_table::{CellInfo, HeaderCellInfo};
use nohash_hasher::IntMap;
use re_format::format_int;
use re_log_types::{EntryId, TimelineName, Timestamp};
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::list_item::ItemButton;
use re_ui::menu::menu_style;
use re_ui::{UiExt as _, icons};
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};
use std::mem;
use std::sync::Arc;

use crate::datafusion_adapter::DataFusionAdapter;
use crate::table_blueprint::{
    ColumnBlueprint, EntryLinksSpec, PartitionLinksSpec, SortBy, SortDirection, TableBlueprint,
};
use crate::table_utils::{ColumnConfig, TableConfig, apply_table_style_fixes, cell_ui, header_ui};
use crate::{DisplayRecordBatch, default_display_name_for_column};

struct Column<'a> {
    /// The ID of the column (based on it's corresponding [`re_sorbet::ColumnDescriptor`]).
    id: egui::Id,

    /// Reference to the descriptor of this column.
    desc: ColumnDescriptorRef<'a>,

    /// The blueprint of this column.
    blueprint: ColumnBlueprint,
}

impl Column<'_> {
    fn display_name(&self) -> String {
        self.blueprint
            .display_name
            .clone()
            .unwrap_or_else(|| default_display_name_for_column(&self.desc))
    }
}

/// Keep track of a [`re_sorbet::SorbetBatch`]'s columns, along with their order and their blueprint.
struct Columns<'a> {
    columns: Vec<Column<'a>>,
    column_from_index: IntMap<egui::Id, usize>,
}

impl<'a> Columns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema, column_blueprint_fn: &ColumnBlueprintFn<'_>) -> Self {
        let (columns, column_from_index) = sorbet_schema
            .columns
            .iter()
            .enumerate()
            .map(|(index, desc)| {
                let id = egui::Id::new(desc);
                let desc = desc.into();
                let blueprint = column_blueprint_fn(&desc);

                let column = Column {
                    id,
                    desc,
                    blueprint,
                };

                (column, (id, index))
            })
            .unzip();

        Self {
            columns,
            column_from_index,
        }
    }
}

impl Columns<'_> {
    fn iter(&self) -> impl Iterator<Item = &Column<'_>> {
        self.columns.iter()
    }

    fn index_from_id(&self, id: Option<egui::Id>) -> Option<usize> {
        id.and_then(|id| self.column_from_index.get(&id).copied())
    }

    fn index_and_column_from_id(&self, id: Option<egui::Id>) -> Option<(usize, &Column<'_>)> {
        id.and_then(|id| self.column_from_index.get(&id).copied())
            .and_then(|index| self.columns.get(index).map(|column| (index, column)))
    }
}

type ColumnBlueprintFn<'a> = Box<dyn Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    /// If provided, add a title UI on top of the table.
    //TODO(ab): for now, this is the only way to have the column visibility/order menu
    title: Option<String>,

    /// User-provided closure to provide column blueprint.
    column_blueprint_fn: ColumnBlueprintFn<'a>,

    /// The blueprint used the first time the table is queried.
    initial_blueprint: TableBlueprint,
}

impl<'a> DataFusionTableWidget<'a> {
    /// Clears all caches related to this session context and table reference.
    pub fn refresh(
        egui_ctx: &egui::Context,
        session_ctx: &SessionContext,
        table_ref: impl Into<TableReference>,
    ) {
        let id = id_from_session_context_and_table(session_ctx, &table_ref.into());

        DataFusionAdapter::clear_state(egui_ctx, id);
    }

    pub fn new(session_ctx: Arc<SessionContext>, table_ref: impl Into<TableReference>) -> Self {
        Self {
            session_ctx,
            table_ref: table_ref.into(),

            title: None,
            column_blueprint_fn: Box::new(|_| ColumnBlueprint::default()),
            initial_blueprint: Default::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    pub fn column_blueprint(
        mut self,
        column_blueprint_fn: impl Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a,
    ) -> Self {
        self.column_blueprint_fn = Box::new(column_blueprint_fn);

        self
    }

    pub fn generate_partition_links(
        mut self,
        column_name: impl Into<String>,
        partition_id_column_name: impl Into<String>,
        origin: re_uri::Origin,
        dataset_id: EntryId,
    ) -> Self {
        self.initial_blueprint.partition_links = Some(PartitionLinksSpec {
            column_name: column_name.into(),
            partition_id_column_name: partition_id_column_name.into(),
            origin,
            dataset_id,
        });

        self
    }

    pub fn generate_entry_links(
        mut self,
        column_name: impl Into<String>,
        entry_id_column_name: impl Into<String>,
        origin: re_uri::Origin,
    ) -> Self {
        self.initial_blueprint.entry_links = Some(EntryLinksSpec {
            column_name: column_name.into(),
            entry_id_column_name: entry_id_column_name.into(),
            origin,
        });

        self
    }

    pub fn filter(mut self, filter: datafusion::prelude::Expr) -> Self {
        self.initial_blueprint.filter = Some(filter);
        self
    }

    fn loading_ui(ui: &mut egui::Ui) {
        Frame::new().inner_margin(16.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading table…");
            });
        });
    }

    pub fn show(
        self,
        viewer_ctx: &ViewerContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
    ) {
        let tokens = ui.tokens();

        let Self {
            session_ctx,
            table_ref,
            title,
            column_blueprint_fn,
            initial_blueprint,
        } = self;

        if !session_ctx
            .table_exist(table_ref.clone())
            .unwrap_or_default()
        {
            Self::loading_ui(ui);
            return;
        }

        // The TableConfig should be persisted across sessions, so we also need a static id.
        let static_id = Id::new(&table_ref);
        let session_id = id_from_session_context_and_table(&session_ctx, &table_ref);

        let table_state = DataFusionAdapter::get(
            runtime,
            ui,
            &session_ctx,
            table_ref.clone(),
            session_id,
            initial_blueprint,
        );

        let requested_sorbet_batches = table_state.requested_sorbet_batches.lock();

        let sorbet_batches = match (
            requested_sorbet_batches.try_as_ref(),
            &table_state.last_sorbet_batches,
        ) {
            (Some(Ok(dataframe)), _) => dataframe,

            (Some(Err(err)), _) => {
                let error = format!("Could not load table: {err}");
                drop(requested_sorbet_batches);

                ui.horizontal(|ui| {
                    ui.error_label(error);

                    if ui
                        .small_icon_button(&re_ui::icons::RESET, "Refresh")
                        .clicked()
                    {
                        // This will trigger a fresh query on the next frame.
                        Self::refresh(ui.ctx(), &session_ctx, table_ref);
                    }
                });
                return;
            }

            (None, Some(last_dataframe)) => {
                // The new dataframe is still processing, but we have the previous one to display for now.
                //TODO(ab): add a progress indicator
                last_dataframe
            }

            (None, None) => {
                // still processing, nothing yet to show
                Self::loading_ui(ui);
                return;
            }
        };

        let (fields, sorbet_schema) = {
            let Some(sorbet_batch) = sorbet_batches.first() else {
                ui.label(egui::RichText::new("This dataset is empty").italics());
                return;
            };

            (sorbet_batch.fields(), sorbet_batch.sorbet_schema())
        };

        let num_rows = sorbet_batches
            .iter()
            .map(|record_batch| record_batch.num_rows() as u64)
            .sum();

        let columns = Columns::from(sorbet_schema, &column_blueprint_fn);

        let display_record_batches = sorbet_batches
            .iter()
            .map(|sorbet_batch| {
                DisplayRecordBatch::try_new(
                    sorbet_batch
                        .all_columns_ref()
                        .zip(columns.iter())
                        .map(|((desc, array), column)| (desc, &column.blueprint, array.clone())),
                )
            })
            .collect::<Result<Vec<_>, _>>();

        let display_record_batches = match display_record_batches {
            Ok(display_record_batches) => display_record_batches,
            Err(err) => {
                //TODO(ab): better error handling?
                ui.error_label(err.to_string());
                return;
            }
        };

        let mut sorted_columns = columns.iter().collect::<Vec<_>>();
        sorted_columns.sort_by_key(|c| c.blueprint.sort_key);
        let mut table_config = TableConfig::get_with_columns(
            ui.ctx(),
            static_id,
            sorted_columns.iter().map(|column| {
                ColumnConfig::new_with_visible(
                    column.id,
                    column.display_name(),
                    column.blueprint.default_visibility,
                )
            }),
        );

        if let Some(title) = title {
            title_ui(ui, &mut table_config, &title);
        }

        apply_table_style_fixes(ui.style_mut());

        let mut new_blueprint = table_state.blueprint().clone();

        let mut table_delegate = DataFusionTableDelegate {
            ctx: viewer_ctx,
            fields,
            display_record_batches: &display_record_batches,
            columns: &columns,
            blueprint: table_state.blueprint(),
            new_blueprint: &mut new_blueprint,
            table_config,
        };

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let spacing = mem::take(&mut ui.spacing_mut().item_spacing.y);

            let visible_columns = table_delegate.table_config.visible_columns().count();
            let total_columns = columns.columns.len();

            let refresh = Self::bottom_bar_ui(
                ui,
                viewer_ctx,
                num_rows,
                total_columns,
                visible_columns,
                table_state.queried_at,
            );

            if refresh {
                Self::refresh(ui.ctx(), &session_ctx, table_ref);
            }

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = spacing;
                egui_table::Table::new()
                    .id_salt(session_id)
                    .columns(
                        table_delegate
                            .table_config
                            .visible_column_ids()
                            .map(|id| egui_table::Column::new(200.0).resizable(true).id(id))
                            .collect::<Vec<_>>(),
                    )
                    .headers(vec![egui_table::HeaderRow::new(
                        tokens.table_header_height(),
                    )])
                    .num_rows(num_rows)
                    .show(ui, &mut table_delegate);
            });
        });

        table_delegate.table_config.store(ui.ctx());
        drop(requested_sorbet_batches);
        if table_state.blueprint() != &new_blueprint {
            table_state.update_query(runtime, ui, new_blueprint);
        }
    }

    fn bottom_bar_ui(
        ui: &mut Ui,
        ctx: &ViewerContext<'_>,
        total_rows: u64,
        total_columns: usize,
        visible_columns: usize,
        queried_at: Timestamp,
    ) -> bool {
        let mut refresh = false;

        let response = Frame::new()
            .fill(ui.tokens().table_header_bg_fill)
            .inner_margin(Margin::symmetric(12, 0))
            .show(ui, |ui| {
                let height = 24.0;
                ui.set_height(height);
                ui.horizontal_centered(|ui| {
                    ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                        ui.tokens().text_subdued;
                    ui.visuals_mut().widgets.active.fg_stroke.color = ui.tokens().text_default;

                    egui::Sides::new().show(
                        ui,
                        |ui| {
                            ui.set_height(height);

                            ui.label("rows:");
                            ui.strong(format_int(total_rows as i64));

                            ui.add_space(16.0);

                            ui.label("columns:");
                            ui.strong(format!(
                                "{} out of {}",
                                format_int(visible_columns as i64),
                                format_int(total_columns as i64)
                            ));
                        },
                        |ui| {
                            ui.set_height(height);
                            if icons::RESET.as_button().ui(ui).clicked() {
                                refresh = true;
                            };

                            re_ui::time::short_duration_ui(
                                ui,
                                queried_at,
                                ctx.app_options().timestamp_format,
                                Ui::strong,
                            );
                            ui.label("Last updated:");
                        },
                    );
                });
            })
            .response;

        ui.painter().hline(
            response.rect.x_range(),
            response.rect.top(),
            Stroke::new(1.0, ui.tokens().table_header_stroke_color),
        );

        refresh
    }
}

fn id_from_session_context_and_table(
    session_ctx: &SessionContext,
    table_ref: &TableReference,
) -> Id {
    egui::Id::new((session_ctx.session_id(), table_ref))
}

fn title_ui<'a>(ui: &mut egui::Ui, table_config: &mut TableConfig, title: &str) {
    Frame::new()
        .inner_margin(Margin {
            top: 16,
            bottom: 12,
            left: 16,
            right: 16,
        })
        .show(ui, |ui| {
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading(RichText::new(title).strong());
                },
                |ui| {
                    table_config.button_ui(ui);
                },
            );
        });
}

struct DataFusionTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    fields: &'a Fields,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    blueprint: &'a TableBlueprint,
    new_blueprint: &'a mut TableBlueprint,
    table_config: TableConfig,
}

impl egui_table::TableDelegate for DataFusionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        let tokens = ui.tokens();

        ui.set_truncate_style();

        let id = self.table_config.visible_column_ids().nth(cell.group_index);

        if let Some((index, column)) = self.columns.index_and_column_from_id(id) {
            let column_dataframe_name = self.fields[index].name();
            let column_display_name = column.display_name();

            let current_sort_direction = self.blueprint.sort_by.as_ref().and_then(|sort_by| {
                (sort_by.column.as_str() == column_dataframe_name).then_some(&sort_by.direction)
            });

            header_ui(ui, true, |ui| {
                egui::Sides::new()
                    .show(
                        ui,
                        |ui| {
                            ui.set_height(ui.tokens().table_content_height());
                            let response = ui.label(
                                egui::RichText::new(column_display_name)
                                    .strong()
                                    .monospace(),
                            );

                            if let Some(dir_icon) = current_sort_direction.map(SortDirection::icon)
                            {
                                ui.add_space(-5.0);
                                ui.small_icon(dir_icon, Some(tokens.table_sort_icon_color));
                            }

                            response
                        },
                        |ui| {
                            ui.set_height(ui.tokens().table_content_height());
                            egui::containers::menu::MenuButton::from_button(
                                ui.small_icon_button_widget(&re_ui::icons::MORE, "More options"),
                            )
                            .config(MenuConfig::new().style(menu_style()))
                            .ui(ui, |ui| {
                                for sort_direction in SortDirection::iter() {
                                    let already_sorted =
                                        Some(&sort_direction) == current_sort_direction;

                                    if ui
                                        .add_enabled_ui(!already_sorted, |ui| {
                                            sort_direction.menu_button(ui)
                                        })
                                        .inner
                                        .clicked()
                                    {
                                        self.new_blueprint.sort_by = Some(SortBy {
                                            column: column_dataframe_name.to_owned(),
                                            direction: sort_direction,
                                        });
                                        ui.close();
                                    }
                                }
                            });
                        },
                    )
                    .0
            })
            .inner
            .on_hover_ui(|ui| {
                column_descriptor_ui(ui, &column.desc);
            });
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        cell_ui(ui, false, |ui| {
            // find record batch
            let mut row_index = cell.row_nr as usize;

            ui.set_truncate_style();

            let id = self.table_config.visible_column_ids().nth(cell.col_nr);

            if let Some(col_idx) = self.columns.index_from_id(id) {
                //TODO(ab): make an utility for that
                for display_record_batch in self.display_record_batches {
                    let row_count = display_record_batch.num_rows();
                    if row_index < row_count {
                        // this is the one
                        let column = &display_record_batch.columns()[col_idx];

                        // TODO(#9029): it is _very_ unfortunate that we must provide a fake timeline, but
                        // avoiding doing so needs significant refactoring work.
                        column.data_ui(
                            self.ctx,
                            ui,
                            &re_viewer_context::external::re_chunk_store::LatestAtQuery::latest(
                                TimelineName::new("unknown"),
                            ),
                            row_index,
                            None,
                        );

                        break;
                    } else {
                        row_index -= row_count;
                    }
                }
            }
        });
    }

    fn default_row_height(&self) -> f32 {
        self.ctx.tokens().table_line_height()
    }
}

fn column_descriptor_ui(ui: &mut egui::Ui, column: &ColumnDescriptorRef<'_>) {
    match *column {
        ColumnDescriptorRef::RowId(desc) => {
            let re_sorbet::RowIdColumnDescriptor { is_sorted } = desc;

            header_property_ui(ui, "Type", "row id");
            header_property_ui(ui, "Sorted", sorted_text(*is_sorted));
        }
        ColumnDescriptorRef::Time(desc) => {
            let re_sorbet::IndexColumnDescriptor {
                timeline,
                datatype,
                is_sorted,
            } = desc;

            header_property_ui(ui, "Type", "index");
            header_property_ui(ui, "Timeline", timeline.name());
            header_property_ui(ui, "Sorted", sorted_text(*is_sorted));
            datatype_ui(ui, &column.display_name(), datatype);
        }
        ColumnDescriptorRef::Component(desc) => {
            let re_sorbet::ComponentColumnDescriptor {
                store_datatype,
                component_type,
                entity_path,
                archetype: archetype_name,
                component: _component,
                is_static,
                is_indicator,
                is_tombstone,
                is_semantically_empty,
            } = desc;

            header_property_ui(ui, "Type", "component");
            header_property_ui(
                ui,
                "Component type",
                component_type.map(|a| a.as_str()).unwrap_or("-"),
            );
            header_property_ui(ui, "Entity path", entity_path.to_string());
            datatype_ui(ui, &column.display_name(), store_datatype);
            header_property_ui(
                ui,
                "Archetype",
                archetype_name.map(|a| a.full_name()).unwrap_or("-"),
            );
            header_property_ui(
                ui,
                "Archetype field",
                desc.component_descriptor().archetype_field_name(),
            );
            header_property_ui(ui, "Static", is_static.to_string());
            header_property_ui(ui, "Indicator", is_indicator.to_string());
            header_property_ui(ui, "Tombstone", is_tombstone.to_string());
            header_property_ui(ui, "Empty", is_semantically_empty.to_string());
        }
    }
}

fn sorted_text(sorted: bool) -> &'static str {
    if sorted { "true" } else { "unknown" }
}

fn header_property_ui(ui: &mut egui::Ui, label: &str, value: impl AsRef<str>) {
    egui::Sides::new().show(ui, |ui| ui.strong(label), |ui| ui.monospace(value.as_ref()));
}

fn datatype_ui(ui: &mut egui::Ui, column_name: &str, datatype: &arrow::datatypes::DataType) {
    egui::Sides::new().show(
        ui,
        |ui| ui.strong("Datatype"),
        |ui| {
            // We don't want the copy button to stand out next to the other properties. The copy
            // icon already indicates that it's a button.
            ui.visuals_mut().widgets.inactive.fg_stroke =
                ui.visuals_mut().widgets.noninteractive.fg_stroke;

            if ui
                .add(
                    egui::Button::image_and_text(
                        re_ui::icons::COPY.as_image(),
                        egui::RichText::new(re_arrow_util::format_data_type(datatype)).monospace(),
                    )
                    .image_tint_follows_text_color(true),
                )
                .clicked()
            {
                ui.ctx().copy_text(format!("{datatype:#?}"));
                re_log::info!("Copied full datatype of column `{column_name}` to clipboard");
            }
        },
    );
}
