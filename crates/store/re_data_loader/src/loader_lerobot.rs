use std::sync::Arc;

use arrow::array::{
    ArrayRef, FixedSizeListArray, Float32Array, Int64Array, ListArray, UInt64Array,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field, FieldRef, Schema, SchemaBuilder};
use re_chunk::{ArrowArray, ChunkId, RowId, TransportChunk};

use re_log_types::external::re_tuid;
use re_types::components::Scalar;
use re_types::{Component as _, Loggable};

use crate::le_robot::LeRobotDatasetMetadata;
use crate::{DataLoader, DataLoaderError, LoadedData};

pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        use std::fs::File;

        use ahash::HashMap;
        use arrow::{
            array::{Float64Array, RecordBatch, StructArray},
            datatypes::{DataType, Field, FieldRef, Fields, SchemaBuilder, SchemaRef},
        };
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use re_arrow_util::ArrowArrayDowncastRef;
        use re_chunk::{
            Chunk, ChunkBuilder, ChunkId, EntityPath, RowId, TimeColumn, TimePoint, Timeline,
        };
        use re_log_types::{external::re_tuid::Tuid, ArrowMsg, TimeType};

        if !crate::le_robot::is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let metadata = LeRobotDatasetMetadata::load_from_directory(&filepath)
            .map_err(|err| DataLoaderError::Other(anyhow::Error::new(err)));

        // load first parquet
        let parquet_reader = parquet::file::metadata::ParquetMetaDataReader::new();
        let file = File::open(filepath.join("data/chunk-000/episode_000000.parquet"))?;
        let data = parquet_reader
            .with_page_indexes(true)
            .parse_and_finish(&file)
            .expect("failed to read parquet data");

        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();

        let chunk_id = Tuid::new();
        let mut reader = builder.build().unwrap();
        let record_batch = reader.next().unwrap().unwrap();

        let actions = split_and_convert_fixed_size_list_column(
            record_batch
                .column_by_name("action")
                .expect("failed to get action"),
        );

        let row_ids = (0..actions.get(0).expect("failed to get first action").len())
            .map(|_| RowId::new())
            .collect::<Vec<_>>();

        // Build the struct column
        let row_ids = <RowId as Loggable>::to_arrow(&row_ids)
            // Unwrap: native RowIds cannot fail to serialize.
            .unwrap();
        let row_ids = Arc::new(
            row_ids
                .downcast_array_ref::<StructArray>()
                // Unwrap: RowId schema is known in advance to be a struct array -- always.
                .unwrap()
                .clone(),
        );

        let frame_indices = Arc::new(
            record_batch
                .column_by_name("frame_index")
                .expect("failed to get frame index")
                .clone(),
        );

        for (idx, action_values) in actions.iter().enumerate() {
            let entity_path = format!("action/{idx}");
            let (schema, data_field_inner) = make_schema_for_entity(entity_path.clone());
            let mut sliced: Vec<ArrayRef> = Vec::new();

            for idx in 0..action_values.len() {
                sliced.push(action_values.slice(idx, 1));
            }

            let data_arrays = sliced.iter().map(|e| Some(e.as_ref())).collect::<Vec<_>>();
            #[allow(clippy::unwrap_used)] // we know we've given the right field type
            let data_field_array: arrow::array::ListArray =
                re_arrow_util::arrow_util::arrays_to_list_array(
                    data_field_inner.data_type().clone(),
                    &data_arrays,
                )
                .unwrap();

            // re_log::info!(
            //     "{}: row_ids: {}, data_field: {}, frame_indices: {}",
            //     entity_path,
            //     row_ids.len(),
            //     data_field_array.len(),
            //     frame_indices.len()
            // );

            let finished_record_batch = RecordBatch::try_new(
                Arc::new(schema),
                vec![
                    row_ids.clone(),
                    Arc::new(data_field_array),
                    frame_indices.clone(),
                ],
            )
            .expect("failed to make batch");

            let msg = ArrowMsg {
                chunk_id,
                timepoint_max: TimePoint::default(),
                batch: finished_record_batch,
                on_release: None,
            };

            tx.send(LoadedData::LogMsg(
                LeRobotDatasetLoader::name(&LeRobotDatasetLoader),
                re_log_types::LogMsg::ArrowMsg(settings.store_id.clone(), msg),
            ))
            .expect("failed to send batch");
        }

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        re_log::info!("loading path: {filepath:?}");
        return Err(DataLoaderError::Incompatible(filepath));
    }
}

fn split_and_convert_fixed_size_list_column(array: &ArrayRef) -> Vec<ArrayRef> {
    let fixed_size_list = array
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .expect("Expected FixedSizeListArray");

    let num_entries = fixed_size_list.len();
    let inner_values = fixed_size_list
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .unwrap();

    (0..19)
        .map(|i| {
            let sliced = inner_values.slice(i, num_entries);
            let float64_values = sliced
                .iter()
                .map(|v| v.map(|f| f as f64)) // Convert float32 -> float64
                .collect::<arrow::array::Float64Array>();

            Arc::new(float64_values) as ArrayRef
        })
        .collect()
}

fn make_schema_for_entity(entity_id: String) -> (Schema, Field) {
    let mut rerun_schema = SchemaBuilder::new();
    rerun_schema
        .metadata_mut()
        .extend(TransportChunk::chunk_metadata_id(ChunkId::new()));
    rerun_schema
        .metadata_mut()
        .extend(TransportChunk::chunk_metadata_entity_path(
            &entity_id.into(),
        ));
    rerun_schema.push(FieldRef::new(make_row_id_column()));

    let data_field_inner = Field::new("item", DataType::Float64, true /* nullable */);

    let data_field = Field::new(
        Scalar::descriptor().component_name.to_string(),
        DataType::List(Arc::new(data_field_inner.clone())),
        false, /* not nullable */
    )
    .with_metadata(TransportChunk::field_metadata_data_column());

    rerun_schema.push(FieldRef::new(data_field.with_metadata({
        let mut metadata = TransportChunk::field_metadata_data_column();
        metadata.extend(TransportChunk::field_metadata_component_descriptor(
            &Scalar::descriptor(),
        ));

        metadata
    })));

    rerun_schema.push(FieldRef::new(
        Field::new("frame_index", DataType::Int64, false /* nullable */).with_metadata({
            let mut metadata = TransportChunk::field_metadata_time_column();

            metadata.extend(TransportChunk::chunk_metadata_is_sorted());
            metadata
        }),
    ));

    (rerun_schema.finish(), data_field_inner)
}

fn make_field_from_hf(field: FieldRef) -> Vec<Field> {
    re_log::info!("processing {:?}", field);

    match field.data_type() {
        arrow::datatypes::DataType::Int64 => {
            vec![make_timeline_column(field)]
        }
        arrow::datatypes::DataType::FixedSizeList(_, num_elements) => {
            make_scalar_columns(field.clone(), *num_elements)
        }
        _ => {
            re_log::error!("unsupported field datatype: {}", field.data_type());
            vec![]
        }
    }
}

fn make_row_id_column() -> Field {
    Field::new(
        RowId::descriptor().to_string(),
        RowId::arrow_datatype().clone(),
        true,
    )
    .with_metadata({
        let mut metadata = TransportChunk::field_metadata_control_column();
        metadata.insert(
            "ARROW:extension:name".to_owned(),
            re_tuid::Tuid::ARROW_EXTENSION_NAME.to_owned(),
        );
        metadata
    })
}

fn make_timeline_column(field: FieldRef) -> Field {
    Field::new(field.name(), field.data_type().clone(), false).with_metadata({
        let mut metadata = TransportChunk::field_metadata_time_column();
        metadata.extend(TransportChunk::field_metadata_is_sorted());
        metadata
    })
}

fn make_scalar_columns(field: FieldRef, num_elements: i32) -> Vec<Field> {
    let mut fields = vec![];
    for idx in 0..num_elements {
        let inner_field = Field::new(
            Scalar::descriptor().component_name.to_string(),
            Scalar::arrow_datatype(),
            true,
        );
        fields.push(
            Field::new(
                format!("{}/{idx}", field.name()),
                arrow::datatypes::DataType::List(FieldRef::new(inner_field)),
                true,
            )
            .with_metadata({
                let mut metadata = TransportChunk::field_metadata_data_column();
                metadata.extend(TransportChunk::field_metadata_component_descriptor(
                    &Scalar::descriptor(),
                ));

                metadata
            }),
        );
    }

    fields
}
