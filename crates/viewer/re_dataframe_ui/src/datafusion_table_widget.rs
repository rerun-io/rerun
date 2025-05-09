use std::sync::Arc;

use arrow::datatypes::Fields;
use datafusion::catalog::TableReference;
use datafusion::prelude::SessionContext;
use egui::{Frame, Id, Margin, RichText};
use egui_table::{CellInfo, HeaderCellInfo};
use nohash_hasher::IntMap;

use re_log_types::{EntryId, TimelineName};
use re_sorbet::{BatchType, ColumnDescriptorRef, SorbetSchema};
use re_ui::UiExt as _;
use re_ui::list_item::ItemButton;
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};

use crate::DisplayRecordBatch;
use crate::datafusion_adapter::DataFusionAdapter;
use crate::table_blueprint::{PartitionLinksSpec, SortBy, SortDirection, TableBlueprint};
use crate::table_utils::{
    CELL_MARGIN, ColumnConfig, TableConfig, apply_table_style_fixes, cell_ui, header_ui,
};

/// Keep track of the columns in a sorbet batch, indexed by id.
//TODO(ab): merge this into `TableConfig` when table config is no longer used elsewhere.
struct Columns<'a> {
    /// Column index and descriptor from id
    inner: IntMap<egui::Id, (usize, ColumnDescriptorRef<'a>)>,
}

impl<'a> Columns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema) -> Self {
        let inner = sorbet_schema
            .columns
            .iter()
            .enumerate()
            .map(|(index, desc)| (egui::Id::new(desc), (index, desc.into())))
            .collect::<IntMap<_, _>>();

        Self { inner }
    }
}

impl Columns<'_> {
    fn index_from_id(&self, id: Option<egui::Id>) -> Option<usize> {
        id.and_then(|id| self.inner.get(&id).map(|(index, _)| *index))
    }

    fn index_and_descriptor_from_id(
        &self,
        id: Option<egui::Id>,
    ) -> Option<(usize, &ColumnDescriptorRef<'_>)> {
        id.and_then(|id| self.inner.get(&id).map(|(index, desc)| (*index, desc)))
    }
}

type ColumnNameFn<'a> = Option<Box<dyn Fn(&ColumnDescriptorRef<'_>) -> String + 'a>>;

type ColumnVisibilityFn<'a> = Option<Box<dyn Fn(&ColumnDescriptorRef<'_>) -> bool + 'a>>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    /// If provided, add a title UI on top of the table.
    //TODO(ab): for now, this is the only way to have the column visibility/order menu
    title: Option<String>,

    /// If provided and if `title` is set, add a button next to the title.
    title_button: Option<Box<dyn ItemButton + 'a>>,

    /// Closure used to determine the display name of the column.
    ///
    /// Defaults to using [`ColumnDescriptorRef::column_name`].
    column_name_fn: ColumnNameFn<'a>,

    /// Closure used to determine the default visibility of the column
    default_column_visibility_fn: ColumnVisibilityFn<'a>,

    /// The blueprint used the first time the table is queried.
    initial_blueprint: TableBlueprint,
}

impl<'a> DataFusionTableWidget<'a> {
    /// Clears all caches related to this session context and table reference.
    pub fn clear_state(
        egui_ctx: &egui::Context,
        session_ctx: &SessionContext,
        table_ref: impl Into<TableReference>,
    ) {
        let id = id_from_session_context_and_table(session_ctx, &table_ref.into());

        TableConfig::clear_state(egui_ctx, id);
        DataFusionAdapter::clear_state(egui_ctx, id);
    }

    pub fn new(session_ctx: Arc<SessionContext>, table_ref: impl Into<TableReference>) -> Self {
        Self {
            session_ctx,
            table_ref: table_ref.into(),

            title: None,
            title_button: None,
            column_name_fn: None,
            default_column_visibility_fn: None,
            initial_blueprint: Default::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    pub fn title_button(mut self, button: impl ItemButton + 'a) -> Self {
        self.title_button = Some(Box::new(button));

        self
    }

    pub fn column_name(
        mut self,
        column_name_fn: impl Fn(&ColumnDescriptorRef<'_>) -> String + 'a,
    ) -> Self {
        self.column_name_fn = Some(Box::new(column_name_fn));

        self
    }

    // TODO(ab): this should best be expressed as part of the `TableBlueprint`, but we need better
    // column selector first.
    pub fn default_column_visibility(
        mut self,
        column_visibility_fn: impl Fn(&ColumnDescriptorRef<'_>) -> bool + 'a,
    ) -> Self {
        self.default_column_visibility_fn = Some(Box::new(column_visibility_fn));

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

    pub fn show(
        self,
        viewer_ctx: &ViewerContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
    ) {
        let Self {
            session_ctx,
            table_ref,
            title,
            title_button,
            column_name_fn,
            default_column_visibility_fn,
            initial_blueprint,
        } = self;

        if !session_ctx
            .table_exist(table_ref.clone())
            .unwrap_or_default()
        {
            // Let's not be too intrusive here, as this can often happen temporarily while the table
            // providers are being registered to the session context after refreshing.
            ui.label(format!(
                "Loading table… (table `{}` not found in session context)",
                &table_ref
            ));
            return;
        }

        let id = id_from_session_context_and_table(&session_ctx, &table_ref);

        let table_state = DataFusionAdapter::get(
            runtime,
            ui,
            &session_ctx,
            table_ref.clone(),
            id,
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

                    if ui.small_icon_button(&re_ui::icons::RESET).clicked() {
                        // This will trigger a fresh query on the next frame.
                        Self::clear_state(ui.ctx(), &session_ctx, table_ref);
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
                ui.label("Loading table…");
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

        let columns = Columns::from(sorbet_schema);

        let display_record_batches = sorbet_batches
            .iter()
            .map(|sorbet_batch| {
                DisplayRecordBatch::try_new(
                    sorbet_batch
                        .all_columns_ref()
                        .map(|(desc, array)| (desc, array.clone())),
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

        let mut table_config = TableConfig::get_with_columns(
            ui.ctx(),
            id,
            sorbet_schema.columns.iter_ref().map(|c| {
                let name = if let Some(column_name_fn) = &column_name_fn {
                    column_name_fn(&c)
                } else {
                    c.column_name(BatchType::Dataframe)
                };

                let visible = if let Some(column_visibility_fn) = &default_column_visibility_fn {
                    column_visibility_fn(&c)
                } else {
                    true
                };

                ColumnConfig::new_with_visible(Id::new(c), name, visible)
            }),
        );

        if let Some(title) = title {
            title_ui(ui, &mut table_config, &title, title_button);
        }

        apply_table_style_fixes(ui.style_mut());

        let mut new_blueprint = table_state.blueprint().clone();

        let mut table_delegate = DataFusionTableDelegate {
            ctx: viewer_ctx,
            fields,
            display_record_batches: &display_record_batches,
            columns: &columns,
            column_name_fn: &column_name_fn,
            blueprint: table_state.blueprint(),
            new_blueprint: &mut new_blueprint,
            table_config,
        };

        egui_table::Table::new()
            .id_salt(id)
            .columns(
                table_delegate
                    .table_config
                    .visible_column_ids()
                    .map(|id| egui_table::Column::new(200.0).resizable(true).id(id))
                    .collect::<Vec<_>>(),
            )
            .headers(vec![egui_table::HeaderRow::new(
                re_ui::DesignTokens::table_header_height() + CELL_MARGIN.sum().y,
            )])
            .num_rows(num_rows)
            .show(ui, &mut table_delegate);

        table_delegate.table_config.store(ui.ctx());
        drop(requested_sorbet_batches);
        table_state.update_query(runtime, ui, new_blueprint);
    }
}

fn id_from_session_context_and_table(
    session_ctx: &SessionContext,
    table_ref: &TableReference,
) -> Id {
    egui::Id::new((session_ctx.session_id(), table_ref))
}

fn title_ui<'a>(
    ui: &mut egui::Ui,
    table_config: &mut TableConfig,
    title: &str,
    title_button: Option<Box<dyn ItemButton + 'a>>,
) {
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
                    if let Some(title_button) = title_button {
                        title_button.ui(ui);
                    }
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
    column_name_fn: &'a ColumnNameFn<'a>,
    blueprint: &'a TableBlueprint,
    new_blueprint: &'a mut TableBlueprint,
    table_config: TableConfig,
}

impl egui_table::TableDelegate for DataFusionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let id = self.table_config.visible_column_ids().nth(cell.group_index);

        if let Some((index, desc)) = self.columns.index_and_descriptor_from_id(id) {
            let column_name = self.fields[index].name();
            let name = if let Some(renamer) = self.column_name_fn {
                renamer(desc)
            } else {
                desc.column_name(BatchType::Dataframe)
            };

            let current_sort_direction = self.blueprint.sort_by.as_ref().and_then(|sort_by| {
                (sort_by.column.as_str() == column_name).then_some(&sort_by.direction)
            });

            header_ui(ui, |ui| {
                egui::Sides::new()
                    .show(
                        ui,
                        |ui| {
                            let response = ui.label(egui::RichText::new(name).strong().monospace());

                            if let Some(dir_icon) = current_sort_direction.map(SortDirection::icon)
                            {
                                ui.add_space(-5.0);
                                ui.small_icon(
                                    dir_icon,
                                    Some(
                                        ui.design_tokens()
                                            .color(re_ui::ColorToken::blue(re_ui::Scale::S450)),
                                    ),
                                );
                            }

                            response
                        },
                        |ui| {
                            egui::containers::menu::MenuButton::from_button(
                                ui.small_icon_button_widget(&re_ui::icons::MORE),
                            )
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
                                            column: column_name.to_owned(),
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
                header_tooltip_ui(ui, desc);
            });
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        cell_ui(ui, |ui| {
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
        re_ui::DesignTokens::table_line_height() + CELL_MARGIN.sum().y
    }
}

fn header_tooltip_ui(ui: &mut egui::Ui, column: &ColumnDescriptorRef<'_>) {
    match column {
        ColumnDescriptorRef::RowId(desc) => {
            header_property_ui(ui, "Type", "row id");
            header_property_ui(ui, "Sorted", sorted_text(desc.is_sorted));
        }
        ColumnDescriptorRef::Time(desc) => {
            header_property_ui(ui, "Type", "index");
            header_property_ui(ui, "Timeline", desc.timeline_name());
            header_property_ui(ui, "Sorted", sorted_text(desc.is_sorted()));
            datatype_ui(ui, desc.datatype());
        }
        ColumnDescriptorRef::Component(desc) => {
            header_property_ui(ui, "Type", "component");
            header_property_ui(ui, "Name", desc.component_name.full_name());
            header_property_ui(ui, "Entity path", desc.entity_path.to_string());
            datatype_ui(ui, &desc.store_datatype);
            header_property_ui(
                ui,
                "Archetype",
                desc.archetype_name.map(|a| a.full_name()).unwrap_or("-"),
            );
            header_property_ui(
                ui,
                "Archetype field",
                desc.archetype_field_name.map(|a| a.as_str()).unwrap_or("-"),
            );
            header_property_ui(ui, "Static", format!("{}", desc.is_static));
            header_property_ui(ui, "Indicator", format!("{}", desc.is_indicator));
            header_property_ui(ui, "Tombstone", format!("{}", desc.is_tombstone));
            header_property_ui(ui, "Empty", format!("{}", desc.is_semantically_empty));
        }
    }
}

fn sorted_text(sorted: bool) -> &'static str {
    if sorted {
        "true"
    } else {
        "unknown"
    }
}

fn header_property_ui(ui: &mut egui::Ui, label: &str, value: impl AsRef<str>) {
    egui::Sides::new().show(ui, |ui| ui.strong(label), |ui| ui.monospace(value.as_ref()));
}

fn datatype_ui(ui: &mut egui::Ui, datatype: &arrow::datatypes::DataType) {
    egui::Sides::new().show(
        ui,
        |ui| ui.strong("Datatype"),
        |ui| {
            if ui
                .button(egui::RichText::new(re_arrow_util::format_data_type(datatype)).monospace())
                .clicked()
            {
                ui.ctx().copy_text(format!("{datatype:#?}"));
            }
        },
    );
}
