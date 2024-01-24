//! This example demonstrates how to implement and register a [`re_data_source::DataLoader`] into
//! the Rerun Viewer in order to add support for loading arbitrary files.
//!
//! Usage:
//! ```sh
//! $ cargo r -p custom_data_loader -- path/to/some/file
//! ```

use rerun::{
    external::{anyhow, re_build_info, re_data_source, re_log, tokio},
    log::{DataRow, RowId},
    EntityPath, TimePoint,
};

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    re_log::setup_logging();

    re_data_source::register_custom_data_loader(HashLoader);

    let build_info = re_build_info::build_info!();
    rerun::run(build_info, rerun::CallSource::Cli, std::env::args())
        .await
        .map(std::process::ExitCode::from)
}

// ---

/// A custom [`re_data_source::DataLoader`] that logs the hash of file as a [`rerun::TextDocument`].
struct HashLoader;

impl re_data_source::DataLoader for HashLoader {
    fn name(&self) -> String {
        "rerun.data_loaders.HashLoader".into()
    }

    fn load_from_path(
        &self,
        _store_id: rerun::external::re_log_types::StoreId,
        path: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<re_data_source::LoadedData>,
    ) -> Result<(), re_data_source::DataLoaderError> {
        let contents = std::fs::read(&path)?;
        if path.is_dir() {
            return Err(re_data_source::DataLoaderError::Incompatible(path)); // simply not interested
        }
        hash_and_log(&tx, &path, &contents)
    }

    fn load_from_file_contents(
        &self,
        _store_id: rerun::external::re_log_types::StoreId,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<re_data_source::LoadedData>,
    ) -> Result<(), re_data_source::DataLoaderError> {
        hash_and_log(&tx, &filepath, &contents)
    }
}

fn hash_and_log(
    tx: &std::sync::mpsc::Sender<re_data_source::LoadedData>,
    filepath: &std::path::Path,
    contents: &[u8],
) -> Result<(), re_data_source::DataLoaderError> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    contents.hash(&mut h);

    let doc = rerun::TextDocument::new(format!("{:08X}", h.finish()))
        .with_media_type(rerun::MediaType::TEXT);

    let entity_path = EntityPath::from_file_path(filepath);
    let entity_path = format!("{entity_path}/hashed").into();
    let row = DataRow::from_archetype(RowId::new(), TimePoint::timeless(), entity_path, &doc)?;

    tx.send(row.into()).ok();

    Ok(())
}
