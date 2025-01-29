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

        use arrow::{
            array::RecordBatch,
            datatypes::{Field, FieldRef, SchemaBuilder, SchemaRef},
        };
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use re_chunk::{RowId, TimePoint};
        use re_log_types::{external::re_tuid::Tuid, ArrowMsg};
        use re_types::Component;

        if !crate::le_robot::is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let metadata = LeRobotDatasetMetadata::load_from_directory(&filepath)
            .map_err(|err| DataLoaderError::Other(anyhow::Error::new(err)));

        re_log::info!("loaded metadata: {metadata:?}");

        // load first parquet
        let parquet_reader = parquet::file::metadata::ParquetMetaDataReader::new();
        let file = File::open(filepath.join("data/chunk-000/episode_000000.parquet"))?;
        let data = parquet_reader
            .with_page_indexes(true)
            .parse_and_finish(&file)
            .expect("failed to read parquet data");

        re_log::info!("schema: {:#?}", data.file_metadata().schema_descr());

        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        println!("Converted arrow schema is: {}", builder.schema());

        let chunk_id = Tuid::new();
        let mut reader = builder.build().unwrap();
        let record_batch = reader.next().unwrap().unwrap();

        let mut new_schema = SchemaBuilder::from(record_batch.schema().fields.clone());
        new_schema
            .metadata_mut()
            .insert("rerun.id".into(), format!("{:X}", chunk_id.as_u128()));
        new_schema
            .metadata_mut()
            .insert("rerun.entity_path".into(), "example_column".into());

        let record_batch = record_batch
            .with_schema(SchemaRef::new(new_schema.finish()))
            .expect("failed to mutate schema");

        let msg = ArrowMsg {
            chunk_id,
            timepoint_max: TimePoint::default(),
            batch: record_batch,
            on_release: None,
        };

        tx.send(LoadedData::LogMsg(
            LeRobotDatasetLoader::name(&LeRobotDatasetLoader),
            re_log_types::LogMsg::ArrowMsg(settings.store_id.clone(), msg),
        ))
        .expect("failed to send batch");

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
