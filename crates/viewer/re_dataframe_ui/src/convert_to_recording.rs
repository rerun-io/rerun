use std::sync::Arc;

use arrow::datatypes::{FieldRef, Schema, SchemaRef};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::{ArrowMsg, EntityPath, LogMsg, SetStoreInfo, StoreId, StoreKind, Timeline};
use re_sorbet::{ColumnDescriptorRef, IndexColumnDescriptor, RowIdColumnDescriptor, SorbetBatch};
use re_types_core::{ChunkId, RowId};
use re_viewer_context::{SystemCommand, SystemCommandSender as _, ViewerContext};

pub fn send_sorbet_batches_as_recording(ctx: &ViewerContext<'_>, sorbet_batches: &[SorbetBatch]) {
    if sorbet_batches.is_empty() {
        // nothing to send
        return;
    }

    // TODO: sdk is wrong here, should probably be a new "FromTable" source
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::Sdk,
    );

    let store_id = StoreId::new(
        StoreKind::Recording,
        "__converted_tables",
        re_log_types::RecordingId::random(),
    );

    if let Err(err) = tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: re_tuid::Tuid::new(),
        info: re_log_types::StoreInfo {
            store_id: store_id.clone(),
            cloned_from: None,
            store_source: re_log_types::StoreSource::Unknown,
            store_version: None,
        },
    })) {
        re_log::warn_once!("could not send set store info message: {err}");
    }

    sorbet_batch_to_chunk_recording_batch(sorbet_batches, |chunk_id, record_batch| {
        if let Err(err) = tx.send(LogMsg::ArrowMsg(
            store_id.clone(),
            ArrowMsg {
                chunk_id: chunk_id.as_tuid(),
                batch: record_batch,
                on_release: None,
            },
        )) {
            re_log::warn_once!("could not send log message: {err}");
        }
    });

    ctx.command_sender()
        .send_system(SystemCommand::AddReceiver(rx));
}

pub fn sorbet_batch_to_chunk_recording_batch(
    sorbet_batches: &[SorbetBatch],
    on_chunk_record_batch: impl Fn(ChunkId, RecordBatch),
) {
    let Some(first_batch) = sorbet_batches.first() else {
        // nothing to send...
        return;
    };

    let schema = first_batch.sorbet_schema();
    let orig_schema = first_batch.schema();

    // The original metadata might have `rerun:sorted` values, but we can't trust it since our
    // sorbet batch might be arbitrarily reordered by the table widget. So we strip that metadata.
    let orig_fields = strip_is_sorted_metadata(orig_schema.fields());

    let mut row_id_column = None;
    let mut index_columns = vec![];
    let mut component_columns: IntMap<EntityPath, Vec<usize>> = Default::default();

    for (col_index, column_descriptor) in schema.columns.iter_ref().enumerate() {
        match column_descriptor {
            ColumnDescriptorRef::RowId(_) => {
                if row_id_column.is_some() {
                    re_log::warn_once!("Unexpected multiple row id columns");
                }

                row_id_column = Some(col_index);
            }

            ColumnDescriptorRef::Time(_) => {
                index_columns.push(col_index);
            }

            ColumnDescriptorRef::Component(component_column_descriptor) => {
                let entity_path = component_column_descriptor.entity_path.clone();
                component_columns
                    .entry(entity_path)
                    .or_default()
                    .push(col_index);
            }
        }
    }

    let record_batch_option = RecordBatchOptions::new();
    let row_index_field = Arc::new(
        IndexColumnDescriptor::from_timeline(Timeline::new_sequence("row_index"), true)
            .to_arrow_field(),
    );

    let mut start_row_index = 0i64;
    for sorbet_batch in sorbet_batches {
        debug_assert_eq!(sorbet_batch.schema(), orig_schema);

        let row_id_field = row_id_column
            .map(|col_idx| Arc::clone(&orig_fields[col_idx]))
            .unwrap_or_else(|| {
                Arc::new(RowIdColumnDescriptor { is_sorted: true }.to_arrow_field())
            });

        let row_count = sorbet_batch.num_rows() as i64;
        let row_index_column: arrow::array::ArrayRef =
            Arc::new(arrow::array::Int64Array::from_iter_values(
                start_row_index..(start_row_index + row_count),
            ));
        start_row_index += row_count;

        #[expect(clippy::iter_over_hash_type)] // we don't really care about chunk order
        for (entity_path, component_column_indices) in &component_columns {
            let chunk_id = ChunkId::new();

            let row_id_column_data = row_id_column
                .map(|col_idx| Arc::clone(sorbet_batch.column(col_idx)))
                .unwrap_or_else(|| {
                    let row_ids = std::iter::from_fn({
                        let tuid: re_tuid::Tuid = *chunk_id;
                        let mut row_id = RowId::from_tuid(tuid.next());
                        move || {
                            let yielded = row_id;
                            row_id = row_id.next();
                            Some(yielded)
                        }
                    })
                    .take(sorbet_batch.num_rows())
                    .collect_vec();

                    Arc::new(RowId::arrow_from_slice(&row_ids))
                });

            let fields = std::iter::once(Arc::clone(&row_id_field))
                .chain(std::iter::once(Arc::clone(&row_index_field)))
                .chain(
                    index_columns
                        .iter()
                        .chain(component_column_indices.iter())
                        .map(|col_idx| Arc::clone(&orig_fields[*col_idx])),
                )
                .collect_vec();

            let mut metadata = orig_schema.metadata().clone();
            metadata.insert("rerun:id".to_owned(), chunk_id.to_string());
            metadata.insert("rerun:entity_path".to_owned(), entity_path.to_string());

            let schema = Schema::new_with_metadata(fields, metadata);

            let column_arrays = std::iter::once(Arc::clone(&row_id_column_data))
                .chain(std::iter::once(Arc::clone(&row_index_column)))
                .chain(
                    index_columns
                        .iter()
                        .chain(component_column_indices.iter())
                        .map(|col_idx| Arc::clone(sorbet_batch.column(*col_idx))),
                )
                .collect();

            let record_batch = arrow::record_batch::RecordBatch::try_new_with_options(
                SchemaRef::new(schema),
                column_arrays,
                &record_batch_option,
            );

            let record_batch = match record_batch {
                Ok(record_batch) => record_batch,
                Err(err) => {
                    re_log::warn_once!("could not send build record batch: {err}");
                    continue;
                }
            };

            on_chunk_record_batch(chunk_id, record_batch);
        }
    }
}

fn strip_is_sorted_metadata(fields: &[FieldRef]) -> Vec<FieldRef> {
    fields
        .iter()
        .map(|field| {
            if field.metadata().contains_key("rerun:is_sorted") {
                let mut field = (**field).clone();
                field.metadata_mut().remove("rerun:is_sorted");
                Arc::new(field)
            } else {
                Arc::clone(field)
            }
        })
        .collect()
}
