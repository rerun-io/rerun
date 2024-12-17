//! This example demonstrates how to implement and register a [`re_data_loader::DataLoader`] into
//! the Rerun Viewer in order to add support for loading arbitrary files.
//!
//! Usage:
//! ```sh
//! $ cargo r -p custom_data_loader -- path/to/some/file
//! ```

use rerun::{
    external::{anyhow, re_build_info, re_data_loader, re_log},
    log::{Chunk, RowId},
    DataLoader as _, EntityPath, LoadedData, TimePoint,
};

fn main() -> anyhow::Result<std::process::ExitCode> {
    let main_thread_token = rerun::MainThreadToken::i_promise_i_am_on_the_main_thread();
    re_log::setup_logging();

    re_data_loader::register_custom_data_loader(HashLoader);

    let build_info = re_build_info::build_info!();
    rerun::run(
        main_thread_token,
        build_info,
        rerun::CallSource::Cli,
        std::env::args(),
    )
    .map(std::process::ExitCode::from)
}

// ---

/// A custom [`re_data_loader::DataLoader`] that logs the hash of file as a [`rerun::TextDocument`].
struct HashLoader;

impl re_data_loader::DataLoader for HashLoader {
    fn name(&self) -> String {
        "rerun.data_loaders.HashLoader".into()
    }

    fn load_from_path(
        &self,
        settings: &rerun::external::re_data_loader::DataLoaderSettings,
        path: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<re_data_loader::LoadedData>,
    ) -> Result<(), re_data_loader::DataLoaderError> {
        let contents = std::fs::read(&path)?;
        if path.is_dir() {
            return Err(re_data_loader::DataLoaderError::Incompatible(path)); // simply not interested
        }
        hash_and_log(settings, &tx, &path, &contents)
    }

    fn load_from_file_contents(
        &self,
        settings: &rerun::external::re_data_loader::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<re_data_loader::LoadedData>,
    ) -> Result<(), re_data_loader::DataLoaderError> {
        hash_and_log(settings, &tx, &filepath, &contents)
    }
}

fn hash_and_log(
    settings: &rerun::external::re_data_loader::DataLoaderSettings,
    tx: &std::sync::mpsc::Sender<re_data_loader::LoadedData>,
    filepath: &std::path::Path,
    contents: &[u8],
) -> Result<(), re_data_loader::DataLoaderError> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    contents.hash(&mut h);

    let doc = rerun::TextDocument::new(format!("{:08X}", h.finish()))
        .with_media_type(rerun::MediaType::TEXT);

    let entity_path = EntityPath::from_file_path(filepath);
    let entity_path = format!("{entity_path}/hashed").into();
    let chunk = Chunk::builder(entity_path)
        .with_archetype(RowId::new(), TimePoint::default(), &doc)
        .build()?;

    let store_id = settings
        .opened_store_id
        .clone()
        .unwrap_or_else(|| settings.store_id.clone());
    let data = LoadedData::Chunk(HashLoader::name(&HashLoader), store_id, chunk);
    tx.send(data).ok();

    Ok(())
}
