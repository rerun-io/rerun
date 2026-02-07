use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{Array as _, RecordBatch as ArrowRecordBatch};
use egui_extras::{Column, TableRow};
use itertools::Itertools as _;
use re_byte_size::SizeBytes;
use re_chunk_store::Chunk;
use re_log_types::{TimeInt, Timeline, TimestampFormat};
use re_ui::{UiExt as _, list_item};

use crate::sort::{SortColumn, SortDirection, sortable_column_header_ui};

/// Any column that can be sorted
#[derive(Default, Clone, Copy, PartialEq)]
enum ChunkColumn {
    #[default]
    RowId,
    Timeline(Timeline),
}

type ChunkSortColumn = SortColumn<ChunkColumn>;

impl ChunkColumn {
    pub(crate) fn ui(&self, ui: &mut egui::Ui, sort_column: &mut ChunkSortColumn) {
        match self {
            Self::RowId => sortable_column_header_ui(self, ui, sort_column, "Row ID"),
            Self::Timeline(timeline) => {
                sortable_column_header_ui(self, ui, sort_column, timeline.name().as_str());
            }
        }
    }
}

pub(crate) struct ChunkUi {
    chunk: Arc<Chunk>,
    sort_column: ChunkSortColumn,
}

impl ChunkUi {
    pub(crate) fn new(chunk: &Arc<Chunk>) -> Self {
        Self {
            chunk: Arc::clone(chunk),
            sort_column: ChunkSortColumn::default(),
        }
    }

    // Return `true` if the user wants to exit the chunk viewer.
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        store: &re_chunk_store::ChunkStore,
    ) -> bool {
        ui.sanity_check();

        let tokens = ui.tokens();

        let table_style = re_ui::TableStyle::Dense;
        let should_exit = self.chunk_info_ui(ui, store);

        //
        // Sort
        //

        // Note: we "physically" sort the chunk according to the sort column. Since chunk cannot
        // be reversed, we must "invert" the index when drawing data if order is descending.

        let chunk = match &self.sort_column.column {
            ChunkColumn::RowId => self.chunk.clone(),
            ChunkColumn::Timeline(timeline) => {
                Arc::new(self.chunk.sorted_by_timeline_if_unsorted(timeline.name()))
            }
        };

        let row_ids = chunk.row_ids_slice();
        let reverse = self.sort_column.direction == SortDirection::Descending;

        //
        // Table
        //

        let time_columns = chunk.timelines().values().collect_vec();

        let components = chunk
            .components()
            .iter()
            .map(|(component, column)| {
                (
                    component,
                    // TODO(#11071): use re_arrow_ui to format the datatype here
                    re_arrow_util::format_data_type(column.list_array.data_type()),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let header_ui = |mut row: TableRow<'_, '_>| {
            row.col(|ui| {
                ChunkColumn::RowId.ui(ui, &mut self.sort_column);
            });

            for time_column in &time_columns {
                row.col(|ui| {
                    ChunkColumn::Timeline(*time_column.timeline()).ui(ui, &mut self.sort_column);
                });
            }

            for (component, datatype) in &components {
                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    let response = ui.button(component.as_str()).on_hover_ui(|ui| {
                        ui.label(format!("{datatype}\n\nClick header to copy"));
                    });

                    if response.clicked() {
                        ui.ctx().copy_text(datatype.clone());
                    }
                });
            }
        };

        //
        // Table
        //

        let row_ui = |mut row: TableRow<'_, '_>| {
            // we handle the sort direction here
            let row_index = if reverse {
                chunk.num_rows() - row.index() - 1
            } else {
                row.index()
            };
            let row_id = row_ids[row_index];

            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                ui.label(row_id.to_string());
            });

            for time_column in &time_columns {
                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    let time = TimeInt::try_from(time_column.times_raw()[row_index])
                        .unwrap_or(TimeInt::STATIC);
                    ui.label(time_column.timeline().typ().format(time, timestamp_format));
                });
            }

            for component in components.keys() {
                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    let component_data = chunk.component_batch_raw(**component, row_index);
                    match component_data {
                        Some(Ok(data)) => {
                            re_arrow_ui::arrow_ui(
                                ui,
                                re_ui::UiLayout::List,
                                timestamp_format,
                                &*data,
                            );
                        }
                        Some(Err(err)) => {
                            ui.error_with_details_on_hover(err.to_string());
                        }
                        None => {
                            ui.weak("-");
                        }
                    }
                });
            }
        };

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                let table_builder = egui_extras::TableBuilder::new(ui)
                    .id_salt(chunk.id())
                    .columns(
                        Column::auto_with_initial_suggestion(200.0).clip(true),
                        1 + time_columns.len() + components.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    .auto_shrink([false, false])
                    .striped(true);

                table_builder
                    .header(tokens.table_row_height(table_style), header_ui)
                    .body(|body| {
                        body.rows(tokens.table_row_height(table_style), row_ids.len(), row_ui);
                    });
            });

        should_exit
    }

    // Returns true if the user wants to exit the chunk viewer.
    fn chunk_info_ui(&self, ui: &mut egui::Ui, store: &re_chunk_store::ChunkStore) -> bool {
        let metadata_ui =
            |ui: &mut egui::Ui, metadata: &std::collections::HashMap<String, String>| {
                for (key, value) in metadata.iter().sorted() {
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new(key).value_text(value),
                    );
                }
            };

        let fields_ui = |ui: &mut egui::Ui, batch: &ArrowRecordBatch| {
            for field in &batch.schema_ref().fields {
                ui.push_id(field.name().clone(), |ui| {
                    ui.list_item_collapsible_noninteractive_label(field.name(), false, |ui| {
                        ui.list_item_collapsible_noninteractive_label("Data type", false, |ui| {
                            // TODO(#11071): use re_arrow_ui to format the datatype here
                            ui.label(re_arrow_util::format_field_datatype(field));
                        });

                        ui.list_item_collapsible_noninteractive_label("Metadata", false, |ui| {
                            metadata_ui(ui, field.metadata());
                        });
                    });
                });
            }
        };

        let chunk_stats_ui =
            |ui: &mut egui::Ui| {
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk ID")
                        .value_text(self.chunk.id().to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Entity")
                        .value_text(self.chunk.entity_path().to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Row count")
                        .value_text(self.chunk.num_rows().to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Heap size").value_text(
                        re_format::format_bytes(
                            <Chunk as SizeBytes>::heap_size_bytes(&self.chunk) as f64
                        ),
                    ),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Sorted")
                        .value_text(if self.chunk.is_sorted() { "yes" } else { "no" }),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Static")
                        .value_text(if self.chunk.is_static() { "yes" } else { "no" }),
                );
            };
        let mut should_exit = false;
        ui.horizontal(|ui| {
            if ui
                .button("Back")
                .on_hover_text("Return to the chunk list view")
                .clicked()
            {
                should_exit = true;
            }

            if ui
                .button("Copy")
                .on_hover_text("Copy this chunk as text")
                .clicked()
            {
                //TODO(#7282): make sure the output is not dependant on the parent terminal's width
                ui.ctx().copy_text(self.chunk.to_string());
            }
        });

        list_item::list_item_scope(ui, "chunk_stats", |ui| {
            ui.list_item_collapsible_noninteractive_label("Stats", false, chunk_stats_ui);
            match self.chunk.to_record_batch() {
                Ok(batch) => {
                    ui.list_item_collapsible_noninteractive_label("Transport", false, |ui| {
                        ui.list_item_collapsible_noninteractive_label("Metadata", false, |ui| {
                            metadata_ui(ui, &batch.schema_ref().metadata);
                        });
                        ui.list_item_collapsible_noninteractive_label("Fields", false, |ui| {
                            fields_ui(ui, &batch);
                        });
                    });
                }
                Err(err) => {
                    ui.error_with_details_on_hover(format!(
                        "Failed to convert to transport: {err}"
                    ));
                }
            }
        });

        ui.info_label(store.format_lineage(&self.chunk.id()));

        should_exit
    }
}
