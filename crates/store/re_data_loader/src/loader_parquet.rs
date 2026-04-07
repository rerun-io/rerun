//! Thin adapter that wraps [`re_parquet`] as a [`DataLoader`].

use crossbeam::channel::Sender;
use re_log_types::StoreId;
use re_quota_channel::send_crossbeam;

use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

const PARQUET_LOADER_NAME: &str = "ParquetLoader";

/// A [`DataLoader`] for generic Parquet files.
///
/// Delegates to [`re_parquet`] for the actual loading logic.
#[derive(Default)]
pub struct ParquetLoader {
    pub config: re_parquet::ParquetConfig,
}

impl DataLoader for ParquetLoader {
    fn name(&self) -> crate::DataLoaderName {
        PARQUET_LOADER_NAME.into()
    }

    fn load_from_path(
        &self,
        settings: &DataLoaderSettings,
        path: std::path::PathBuf,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !path.is_file() || !has_parquet_extension(&path) {
            return Err(DataLoaderError::Incompatible(path));
        }

        re_tracing::profile_function!();

        let config = self.config.clone();
        let prefix = settings
            .entity_path_prefix
            .clone()
            .unwrap_or_else(re_parquet::ParquetConfig::default_entity_path_prefix);
        let store_id = settings.opened_store_id_or_recommended();

        std::thread::Builder::new()
            .name(format!("load_parquet({path:?})"))
            .spawn(
                move || match re_parquet::load_parquet(&path, &config, &prefix) {
                    Ok(chunks) => forward_chunks(chunks, &tx, &store_id),
                    Err(err) => re_log::error!("Failed to load Parquet: {err}"),
                },
            )
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !has_parquet_extension(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        re_tracing::profile_function!();

        let contents = contents.into_owned();
        let config = self.config.clone();
        let prefix = settings
            .entity_path_prefix
            .clone()
            .unwrap_or_else(re_parquet::ParquetConfig::default_entity_path_prefix);
        let store_id = settings.opened_store_id_or_recommended();

        std::thread::Builder::new()
            .name(format!("load_parquet({filepath:?})"))
            .spawn(
                move || match re_parquet::load_parquet_from_bytes(&contents, &config, &prefix) {
                    Ok(chunks) => forward_chunks(chunks, &tx, &store_id),
                    Err(err) => re_log::error!("Failed to load Parquet: {err}"),
                },
            )
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }
}

/// Forward chunks from a [`re_parquet`] iterator to the [`DataLoader`] channel.
///
/// Sends a `SetStoreInfo` message first (consistent with other loaders),
/// then wraps each chunk in [`LoadedData::Chunk`] and sends via `send_crossbeam`.
fn forward_chunks(
    chunks: impl Iterator<Item = Result<re_chunk::Chunk, re_parquet::ParquetError>>,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
) {
    let store_info_msg = crate::prepare_store_info(store_id, re_log_types::FileSource::Sdk);
    if send_crossbeam(
        tx,
        LoadedData::LogMsg(PARQUET_LOADER_NAME.to_owned(), store_info_msg),
    )
    .is_err()
    {
        return;
    }

    for chunk_result in chunks {
        match chunk_result {
            Ok(chunk) => {
                if send_crossbeam(
                    tx,
                    LoadedData::Chunk(PARQUET_LOADER_NAME.to_owned(), store_id.clone(), chunk),
                )
                .is_err()
                {
                    break;
                }
            }
            Err(err) => {
                re_log::error!("Parquet error: {err}");
            }
        }
    }
}

fn has_parquet_extension(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("parquet"))
}

#[cfg(test)]
#[expect(clippy::disallowed_methods)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{Float64Array, RecordBatch};
    use arrow::datatypes::{DataType, Field, Schema};
    use re_chunk::EntityPath;

    use crate::{DataLoader as _, DataLoaderSettings, LoadedData};

    use super::*;

    fn write_parquet_tmp(batch: &RecordBatch) -> std::path::PathBuf {
        use parquet::arrow::ArrowWriter;

        let dir = std::env::temp_dir().join("rerun_parquet_tests");
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join(format!("{}.parquet", re_chunk::ChunkId::new()));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = ArrowWriter::try_new(file, batch.schema(), None).unwrap();
        writer.write(batch).unwrap();
        writer.close().unwrap();

        path
    }

    #[test]
    fn incompatible_extension_rejected() {
        let loader = ParquetLoader::default();
        let (tx, _rx) = crossbeam::channel::bounded(1024);
        let settings = DataLoaderSettings::recommended("test");

        let result = loader.load_from_path(&settings, "data.csv".into(), tx);
        assert!(matches!(
            result,
            Err(crate::DataLoaderError::Incompatible(_))
        ));
    }

    #[test]
    fn parquet_loader_smoke_test() {
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
            ])),
            vec![
                Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
                Arc::new(Float64Array::from(vec![4.0, 5.0, 6.0])),
            ],
        )
        .unwrap();

        let path = write_parquet_tmp(&batch);
        let loader = ParquetLoader::default();
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = DataLoaderSettings::recommended("test");

        loader
            .load_from_path(&settings, path, tx)
            .expect("load should succeed");

        let chunks: Vec<_> = rx
            .iter()
            .filter_map(LoadedData::into_chunk)
            .filter(|c| c.entity_path() != &EntityPath::properties())
            .collect();

        assert!(!chunks.is_empty(), "should produce at least one data chunk");
    }
}
