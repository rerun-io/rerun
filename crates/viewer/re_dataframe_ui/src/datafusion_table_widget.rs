use std::sync::Arc;

use datafusion::catalog::TableReference;
use datafusion::prelude::SessionContext;
use egui::{Frame, Id, Margin, RichText};
use egui_table::{CellInfo, HeaderCellInfo};
use nohash_hasher::IntMap;

use re_log_types::{EntryId, TimelineName};
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::list_item::ItemButton;
use re_ui::UiExt as _;
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};

use crate::datafusion_adapter::{
    DataFusionAdapter, PartitionLinksSpec, SortBy, SortDirection, TableBlueprint,
};
use crate::table_utils::{
    apply_table_style_fixes, cell_ui, header_ui, ColumnConfig, TableConfig, CELL_MARGIN,
};
use crate::DisplayRecordBatch;

/// Keep track of the columns in a sorbet batch, indexed by id.
struct Columns<'a> {
    /// Column index and descriptor from id
    inner: IntMap<egui::Id, (usize, ColumnDescriptorRef<'a>)>,
}

impl<'a> Columns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema) -> Self {
        let inner = sorbet_schema
            .columns
            .descriptors()
            .enumerate()
            .map(|(index, desc)| (egui::Id::new(&desc), (index, desc)))
            .collect::<IntMap<_, _>>();

        Self { inner }
    }
}

impl Columns<'_> {
    fn descriptors(&self) -> impl Iterator<Item = &ColumnDescriptorRef<'_>> {
        self.inner.values().map(|(_, desc)| desc)
    }

    fn index_from_id(&self, id: Option<egui::Id>) -> Option<usize> {
        id.and_then(|id| self.inner.get(&id).map(|(index, _)| *index))
    }

    fn descriptor_from_id(&self, id: Option<egui::Id>) -> Option<&ColumnDescriptorRef<'_>> {
        id.and_then(|id| self.inner.get(&id).map(|(_, desc)| desc))
    }
}

type ColumnRenamerFn<'a> = Option<Box<dyn Fn(&ColumnDescriptorRef<'_>) -> String + 'a>>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,
    id: egui::Id,
    table_ref: TableReference,

    /// If provided, add a title UI on top of the table.
    //TODO(ab): for now, this is the only way to have the column visibility/order menu
    title: Option<String>,

    /// If provided and if `title` is set, add a button next to the title.
    title_button: Option<Box<dyn ItemButton + 'a>>,

    /// Closure used to determine the display name of the column.
    ///
    /// Defaults to using [`ColumnDescriptorRef::name`].
    column_renamer: ColumnRenamerFn<'a>,

    /// The blueprint used the first time the table is queried.
    initial_blueprint: TableBlueprint,

    /// If `true`, force invalidating all caches and refreshing the queries.
    refresh: bool,
}

impl<'a> DataFusionTableWidget<'a> {
    pub fn new(
        session_ctx: Arc<SessionContext>,
        id: impl Into<egui::Id>,
        table_ref: impl Into<TableReference>,
    ) -> Self {
        Self {
            session_ctx,
            id: id.into(),
            table_ref: table_ref.into(),

            title: None,
            title_button: None,
            column_renamer: None,
            initial_blueprint: Default::default(),
            refresh: false,
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

    pub fn column_renamer(
        mut self,
        renamer: impl Fn(&ColumnDescriptorRef<'_>) -> String + 'a,
    ) -> Self {
        self.column_renamer = Some(Box::new(renamer));

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

    pub fn refresh(mut self, refresh: bool) -> Self {
        self.refresh = refresh;

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
            id,
            table_ref,
            title,
            title_button,
            column_renamer,
            initial_blueprint,
            refresh,
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

        // The cache must be invalidated as soon as the input table name or session context change,
        // so we add that to the id.
        let id = id
            .with((&table_ref, session_ctx.session_id()))
            .with("__table_ui_table_state");

        let table_state = DataFusionAdapter::get(
            runtime,
            ui,
            &session_ctx,
            table_ref,
            id,
            initial_blueprint,
            refresh,
        );

        let dataframe = table_state.dataframe.lock();

        let sorbet_batches = match (dataframe.try_as_ref(), &table_state.last_dataframe) {
            (Some(Ok(dataframe)), _) => dataframe,

            (Some(Err(err)), _) => {
                let error = format!("Could not load table: {err}");
                drop(dataframe);

                ui.horizontal(|ui| {
                    ui.error_label(error);

                    if ui.small_icon_button(&re_ui::icons::RESET).clicked() {
                        table_state.clear(ui);
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

        let sorbet_schema = {
            let Some(sorbet_batch) = sorbet_batches.first() else {
                ui.label(egui::RichText::new("This dataset is empty").italics());
                return;
            };

            sorbet_batch.sorbet_schema()
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
                        .all_columns()
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
            columns.descriptors().map(|c| {
                let name = if let Some(renamer) = &column_renamer {
                    renamer(c)
                } else {
                    c.name().to_owned()
                };

                ColumnConfig::new(Id::new(c), name)
            }),
            refresh,
        );

        if let Some(title) = title {
            title_ui(ui, &mut table_config, &title, title_button);
        }

        apply_table_style_fixes(ui.style_mut());

        let mut new_blueprint = table_state.blueprint().clone();

        let mut table_delegate = DataFusionTableDelegate {
            ctx: viewer_ctx,
            display_record_batches: &display_record_batches,
            columns: &columns,
            column_renamer: &column_renamer,
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
        drop(dataframe);
        table_state.update_query(runtime, ui, new_blueprint);
    }
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
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    column_renamer: &'a ColumnRenamerFn<'a>,
    blueprint: &'a TableBlueprint,
    new_blueprint: &'a mut TableBlueprint,
    table_config: TableConfig,
}

impl egui_table::TableDelegate for DataFusionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let id = self.table_config.visible_column_ids().nth(cell.group_index);

        if let Some(desc) = self.columns.descriptor_from_id(id) {
            let column_name = desc.name();
            let name = if let Some(renamer) = self.column_renamer {
                renamer(desc)
            } else {
                desc.name().to_owned()
            };

            let current_sort_direction = self.blueprint.sort_by.as_ref().and_then(|sort_by| {
                (sort_by.column.as_str() == column_name).then_some(&sort_by.direction)
            });

            header_ui(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.label(egui::RichText::new(name).strong().monospace());

                        if let Some(dir_icon) = current_sort_direction.map(SortDirection::icon) {
                            ui.add_space(-5.0);
                            ui.small_icon(
                                dir_icon,
                                Some(
                                    re_ui::design_tokens()
                                        .color(re_ui::ColorToken::blue(re_ui::Scale::S450)),
                                ),
                            );
                        }
                    },
                    |ui| {
                        egui::menu::menu_custom_button(
                            ui,
                            ui.small_icon_button_widget(&re_ui::icons::MORE),
                            |ui| {
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
                                        ui.close_menu();
                                    }
                                }
                            },
                        );
                    },
                );
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
