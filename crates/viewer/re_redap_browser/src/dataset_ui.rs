use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, ListArray as ArrowListArray, StringArray as ArrowStringArray,
};
use arrow::datatypes::{DataType as ArrowDataType, Field as ArrowField};
use egui_table::{CellInfo, HeaderCellInfo};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_types::{EntityPath, TimelineName};
use re_protos::common::v1alpha1::ext::EntryId;
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_sorbet::{ColumnDescriptorRef, ComponentColumnDescriptor, SorbetBatch};
use re_types_core::arrow_helpers::as_array_ref;
use re_ui::UiExt as _;
use re_view_dataframe::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
use re_viewer_context::ViewerContext;

use super::servers::Command;
use crate::context::Context;
use crate::entries::Dataset;

#[derive(thiserror::Error, Debug)]
enum CollectionUiError {
    #[error(transparent)]
    DisplayRecordBatchError(#[from] DisplayRecordBatchError),

    #[error("Unexpected data error: {0}")]
    UnexpectedDataError(String),
}

pub fn dataset_ui(
    viewer_ctx: &ViewerContext<'_>,
    ctx: &Context<'_>,
    ui: &mut egui::Ui,
    origin: &re_uri::Origin,
    dataset: &Dataset,
) {
    let sorbet_schema = {
        let Some(sorbet_batch) = dataset.partition_table.first() else {
            ui.label(egui::RichText::new("This dataset is empty").italics());
            return;
        };

        sorbet_batch.sorbet_schema()
    };

    // The table id mainly drives column widths, along with the id of each column.
    let table_id_salt = egui::Id::new(dataset.id()).with("__dataset_table__");

    let num_rows = dataset
        .partition_table
        .iter()
        .map(|record_batch| record_batch.num_rows() as u64)
        .sum();

    let columns = sorbet_schema
        .columns
        .descriptors()
        .chain(std::iter::once(component_uri_descriptor()))
        .collect::<Vec<_>>();

    //TODO(ab): better column order?

    let display_record_batches: Result<Vec<_>, _> = dataset
        .partition_table
        .iter()
        .map(|sorbet_batch| {
            catalog_sorbet_batch_to_display_record_batch(origin, dataset.id(), sorbet_batch)
        })
        .collect();

    let display_record_batches = match display_record_batches {
        Ok(display_record_batches) => display_record_batches,
        Err(err) => {
            //TODO(ab): better error handling?
            ui.error_label(err.to_string());
            return;
        }
    };

    let mut table_delegate = CollectionTableDelegate {
        ctx: viewer_ctx,
        display_record_batches: &display_record_batches,
        selected_columns: &columns,
    };

    egui::Frame::new().inner_margin(5.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                let _ = ctx.command_sender.send(Command::DeselectEntry);
            }

            if ui.button("Refresh").clicked() {
                let _ = ctx
                    .command_sender
                    .send(Command::RefreshCollection(origin.clone()));
            }
        });

        egui_table::Table::new()
            .id_salt(table_id_salt)
            .columns(
                columns
                    .iter()
                    .map(|field| {
                        egui_table::Column::new(200.0)
                            .resizable(true)
                            .id(egui::Id::new(field))
                    })
                    .collect::<Vec<_>>(),
            )
            .headers(vec![egui_table::HeaderRow::new(
                re_ui::DesignTokens::table_header_height(),
            )])
            .num_rows(num_rows)
            .show(ui, &mut table_delegate);
    });
}

/// Descriptor for the generated `RecordingUri` component.
fn component_uri_descriptor() -> ColumnDescriptorRef<'static> {
    static COMPONENT_URI_DESCRIPTOR: once_cell::sync::Lazy<ComponentColumnDescriptor> =
        once_cell::sync::Lazy::new(|| ComponentColumnDescriptor {
            store_datatype: ArrowDataType::Utf8,
            component_name: "recording_uri".into(),
            entity_path: EntityPath::root(),
            archetype_name: None,
            archetype_field_name: None,
            is_static: false,
            is_indicator: false,
            is_tombstone: false,
            is_semantically_empty: false,
        });

    (&*COMPONENT_URI_DESCRIPTOR).into()
}

/// Convert a `SorbetBatch` to a `DisplayRecordBatch` and generate a `RecordingUri` column on the
/// fly.
fn catalog_sorbet_batch_to_display_record_batch(
    origin: &re_uri::Origin,
    dataset_id: EntryId,
    sorbet_batch: &SorbetBatch,
) -> Result<DisplayRecordBatch, CollectionUiError> {
    let rec_ids = sorbet_batch
        .column_by_name(DATASET_MANIFEST_ID_FIELD_NAME)
        .map(|rec_ids| {
            let list_array = rec_ids
                .downcast_array_ref::<ArrowListArray>()
                .ok_or_else(|| {
                    CollectionUiError::UnexpectedDataError(format!(
                        "{DATASET_MANIFEST_ID_FIELD_NAME} column is not a list array as expected"
                    ))
                })?;

            let recording_uri_arrays = (0..list_array.len())
                .map(|idx| {
                    let list = list_array.value(idx);

                    let string_array =
                        list.downcast_array_ref::<ArrowStringArray>()
                            .ok_or_else(|| {
                                CollectionUiError::UnexpectedDataError(format!(
                                    "{DATASET_MANIFEST_ID_FIELD_NAME} column inner item is not a string \
                                     array as expected"
                                ))
                            })?;
                    let partition_id = string_array.value(0);
                    let dataset_id = dataset_id.id.to_string();

                    let recording_uri = format!("{origin}/dataset/{dataset_id}/data?partition_id={partition_id}");

                    Ok(as_array_ref(ArrowStringArray::from(vec![recording_uri])))
                })
                .collect::<Result<Vec<_>, CollectionUiError>>()?;

            let recording_id_arrays = recording_uri_arrays
                .iter()
                .map(|e| Some(e.as_ref()))
                .collect::<Vec<_>>();

            let rec_id_field = ArrowField::new("item", ArrowDataType::Utf8, true);
            #[allow(clippy::unwrap_used)] // we know we've given the right field type
            let uris = re_arrow_util::arrays_to_list_array(
                rec_id_field.data_type().clone(),
                &recording_id_arrays,
            )
            .expect("We know the datatype is correct");

            Result::<_, CollectionUiError>::Ok((
                component_uri_descriptor(),
                Arc::new(uris) as ArrayRef,
            ))
        })
        .transpose()?;

    DisplayRecordBatch::try_new(
        sorbet_batch
            .all_columns()
            .map(|(desc, array)| (desc, array.clone()))
            .chain(rec_ids),
    )
    .map_err(Into::into)
}

struct CollectionTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    selected_columns: &'a Vec<ColumnDescriptorRef<'a>>,
}

impl egui_table::TableDelegate for CollectionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let name = self.selected_columns[cell.group_index].name();
        let name = name
            .strip_prefix("rerun_")
            .unwrap_or(name.as_str())
            .replace('_', " ");

        ui.strong(name);
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        // find record batch
        let mut row_index = cell.row_nr as usize;

        ui.set_truncate_style();

        for display_record_batch in self.display_record_batches {
            let row_count = display_record_batch.num_rows();
            if row_index < row_count {
                // this is the one
                let column = &display_record_batch.columns()[cell.col_nr];

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

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}
