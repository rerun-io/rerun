use std::sync::Arc;

use datafusion::prelude::SessionContext;
use egui::Id;
use egui_table::{CellInfo, HeaderCellInfo};
use nohash_hasher::IntMap;

use re_log_types::TimelineName;
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::UiExt as _;
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};

use crate::datafusion_adapter::{DataFusionAdapter, DataFusionQuery, SortBy, SortDirection};
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

//TODO:
// - expose a "refresh" functionality
// - expose a column name customisation functionality
// - document that the caller is 100% responsible for NOT calling this function if `table_name` is
//   not yet registered in `session_ctx`.
pub fn table_ui(
    viewer_ctx: &ViewerContext<'_>,
    runtime: &AsyncRuntimeHandle,
    ui: &mut egui::Ui,
    session_ctx: &Arc<SessionContext>,
    origin: &re_uri::Origin,
    table_name: &str,
) {
    let table_id_salt = egui::Id::new((origin, table_name)).with("__table_ui_table_state");

    let table_state = DataFusionAdapter::get(runtime, ui, session_ctx, table_name, table_id_salt);

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
            ui.label("Loading table...");
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

    let table_config = TableConfig::get_with_columns(
        ui.ctx(),
        table_id_salt,
        columns.descriptors().map(|c| {
            //TODO(ab): we should remove this name facility if we don't use it
            ColumnConfig::new(Id::new(c), "unused".to_owned())
        }),
    );

    apply_table_style_fixes(ui.style_mut());

    let mut new_blueprint = table_state.query.clone();

    let mut table_delegate = CollectionTableDelegate {
        ctx: viewer_ctx,
        display_record_batches: &display_record_batches,
        columns: &columns,
        blueprint: &table_state.query,
        new_blueprint: &mut new_blueprint,
        table_config,
    };

    egui_table::Table::new()
        .id_salt(table_id_salt)
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

    drop(dataframe);

    table_state.update_query(runtime, ui, new_blueprint);
}

struct CollectionTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    blueprint: &'a DataFusionQuery,
    new_blueprint: &'a mut DataFusionQuery,
    table_config: TableConfig,
}

impl egui_table::TableDelegate for CollectionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let id = self.table_config.visible_column_ids().nth(cell.group_index);

        if let Some(desc) = self.columns.descriptor_from_id(id) {
            let name = desc.name();

            let sort_direction_icon = self
                .blueprint
                .sort_by
                .as_ref()
                .and_then(|sort_by| (sort_by.column.as_str() == name).then_some(&sort_by.direction))
                .map(SortDirection::icon);

            header_ui(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.label(egui::RichText::new(name).strong().monospace());

                        if let Some(dir_icon) = sort_direction_icon {
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
                                if ui.button("Ascending").clicked() {
                                    self.new_blueprint.sort_by = Some(SortBy {
                                        column: name.to_owned(),
                                        direction: SortDirection::Ascending,
                                    });
                                    ui.close_menu();
                                }

                                if ui.button("Descending").clicked() {
                                    self.new_blueprint.sort_by = Some(SortBy {
                                        column: name.to_owned(),
                                        direction: SortDirection::Descending,
                                    });

                                    ui.close_menu();
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
