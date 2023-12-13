use std::borrow::Cow;
use std::sync::Arc;

use once_cell::sync::Lazy;

use re_log_types::{ArrowMsg, DataRow, FileSource, LogMsg};
use re_smart_channel::Sender;

// ---

/// Synchronously checks whether the file exists and can potentially be loaded, beyond that all
/// errors are asynchronous and handled directly by the [`crate::DataLoader`]s themselves (as in: logged).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_file(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &Sender<LogMsg>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!(path.to_string_lossy());

    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path does not exist: {path:?}",
        )
        .into());
    }

    re_log::info!("Loading {path:?}…");

    let store_info = prepare_store_info(store_id, file_source, path, path.is_dir());
    if let Some(store_info) = store_info {
        if tx.send(store_info).is_err() {
            return Ok(()); // other end has hung up.
        }
    }

    let data = load(store_id, path, path.is_dir(), None)?;
    send(store_id, data, tx);

    Ok(())
}

/// Synchronously checks the file can potentially be loaded, beyond that all errors are
/// asynchronous and handled directly by the [`crate::DataLoader`]s themselves (as in: logged).
// TODO: path is never actually "used"!
pub fn load_from_file_contents(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    contents: std::borrow::Cow<'_, [u8]>,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &Sender<LogMsg>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!(path.to_string_lossy());

    re_log::info!("Loading {path:?}…");

    let store_info = prepare_store_info(store_id, file_source, path, false);
    if let Some(store_info) = store_info {
        if tx.send(store_info).is_err() {
            return Ok(()); // other end has hung up.
        }
    }

    let data = load(store_id, path, false, Some(contents))?;
    send(store_id, data, tx);

    Ok(())
}

// ---

// TODO: support matrix:
// - platform
// - open/dragndrop/cli
// - single/many/folder

// TODO: ZipLoader???

// TODO: docs:
// - stdio (update SDK operating modes?)
// - file loaders
// - file plugins / binary-on-path (-> example)

// TODO: what about directories?
// TODO: what about patterns, e.g. regexes?

// TODO: custom_data_loader example?
// TODO: all of this is designed to run asynchronously, you're all responsible for your errors
//          -> AsyncDataLoader
// TODO: i do think we can get away with not returning any errors
// TODO: you might be asked to load stuff you don't care about!
// TODO: URI loader
// TODO: explain MT in the docs
// TODO: should that actually be a streaming interface kinda thing...?
// TODO:
// - what it is
// - error management
// - how directories are handled
// - how duplicate loaders are handled (flexibility over everything else)
pub trait DataLoader: Send + Sync {
    fn name(&self) -> String;

    // TODO: the store_id corresponds to the shared recording, if plugins want to log to the same
    // shared place.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_file(
        &self,
        // TODO: Explain why the store_id is optional and what kinda stuff you can do with it.
        store_id: re_log_types::StoreId,
        // TODO: a URI in the future
        // TODO: not necessarily a file btw
        path: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
        // TODO: remove error
    ) -> Result<(), DataLoaderError>;

    // TODO
    fn load_from_file_contents(
        &self,
        store_id: re_log_types::StoreId,
        path: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
        // TODO: remove error
    ) -> Result<(), DataLoaderError>;
}

/// Errors that might happen when loading data through a [`DataLoader`].
#[derive(thiserror::Error, Debug)]
pub enum DataLoaderError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Arrow(#[from] re_log_types::DataCellError),

    #[error(transparent)]
    Decode(#[from] re_log_encoding::decoder::DecodeError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// What [`DataLoader`]s load.
///
/// This makes it trivial for [`DataLoader`]s to build the data in whatever form is
/// most convenient for them, whether it is raw components, arrow chunks or even
/// full-on [`LogMsg`]s.
pub enum LoadedData {
    DataRow(DataRow),
    ArrowMsg(ArrowMsg),
    LogMsg(LogMsg),
}

impl From<DataRow> for LoadedData {
    fn from(value: DataRow) -> Self {
        Self::DataRow(value)
    }
}

impl From<ArrowMsg> for LoadedData {
    fn from(value: ArrowMsg) -> Self {
        LoadedData::ArrowMsg(value)
    }
}

impl From<LogMsg> for LoadedData {
    fn from(value: LogMsg) -> Self {
        LoadedData::LogMsg(value)
    }
}

// ---

mod loader_archetype;
mod loader_directory;
mod loader_rrd;

#[cfg(not(target_arch = "wasm32"))]
mod loader_external;

pub use self::loader_archetype::ArchetypeLoader;
pub use self::loader_directory::DirectoryLoader;
pub use self::loader_rrd::RrdDataLoader;

#[cfg(not(target_arch = "wasm32"))]
pub use self::loader_external::{ExternalDataLoader, EXTERNAL_DATA_LOADER_PREFIX};

// ---

/// Keeps track of all builtin [`DataLoader`]s.
///
/// Lazy initialized the first time a file is opened.
pub static BUILTIN_LOADERS: Lazy<Vec<Arc<dyn DataLoader>>> = Lazy::new(|| {
    vec![
        Arc::new(RrdDataLoader) as Arc<dyn DataLoader>,
        Arc::new(ArchetypeLoader),
        Arc::new(DirectoryLoader),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(ExternalDataLoader),
    ]
});

// TODO: user-defined builtin loader

/// Keeps track of the executable path of all external [`DataLoader`]s.
///
/// Lazy initialized the first time a file is opened by running a full scan of the `$PATH`.
///
/// External loaders are _not_ registered on a per-extension basis: we want users to be able to
/// filter data on a much more fine-grained basis that just file extensions (e.g. checking the file
/// itself for magic bytes).
#[cfg(not(target_arch = "wasm32"))]
pub static EXTERNAL_LOADERS: Lazy<Vec<std::path::PathBuf>> = Lazy::new(|| {
    re_tracing::profile_function!();

    use walkdir::WalkDir;

    let dirpaths = std::env::var("PATH")
        .ok()
        .into_iter()
        .flat_map(|paths| paths.split(':').map(ToOwned::to_owned).collect::<Vec<_>>())
        .map(std::path::PathBuf::from);

    let executables: ahash::HashSet<_> = dirpaths
        .into_iter()
        .flat_map(|dirpath| {
            WalkDir::new(dirpath).into_iter().filter_map(|entry| {
                let Ok(entry) = entry else {
                    // TODO: explai
                    return None;
                };
                let filepath = entry.path();
                let is_rerun_loader = filepath.file_name().map_or(false, |filename| {
                    filename
                        .to_string_lossy()
                        .starts_with(EXTERNAL_DATA_LOADER_PREFIX)
                });
                (filepath.is_file() && is_rerun_loader).then(|| filepath.to_owned())
            })
        })
        .collect();

    // NOTE: We call all available loaders and do so in parallel: order is irrelevant here.
    executables.into_iter().collect()
});

// ---

/// Empty string if not extension.
#[inline]
fn extension(path: &std::path::Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string()
}

/// Returns whether the given path is supported by builtin [`DataLoader`]s.
///
/// This does _not_ access the filesystem.
#[inline]
fn is_builtin(path: &std::path::Path, is_dir: bool) -> bool {
    is_dir || crate::is_known_file_extension(&extension(path))
}

/// Prepares an adequate [`re_log_types::StoreInfo`] [`LogMsg`] given the input.
fn prepare_store_info(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    is_dir: bool,
) -> Option<LogMsg> {
    re_tracing::profile_function!(path.display().to_string());

    use re_log_types::SetStoreInfo;

    let app_id = re_log_types::ApplicationId(path.display().to_string());
    let store_source = re_log_types::StoreSource::File { file_source };

    let is_builtin = is_builtin(path, is_dir);
    let is_rrd = crate::SUPPORTED_RERUN_EXTENSIONS.contains(&extension(path).as_str());

    (!is_rrd && is_builtin).then(|| {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: re_log_types::RowId::new(),
            info: re_log_types::StoreInfo {
                application_id: app_id.clone(),
                store_id: store_id.clone(),
                is_official_example: false,
                started: re_log_types::Time::now(),
                store_source,
                store_kind: re_log_types::StoreKind::Recording,
            },
        })
    })
}

/// Loads the data at `path` using all available [`DataLoader`]s, builtin and external.
///
/// Returns a channel with all the [`LoadedData`]:
/// - On native, this is filled asynchronously from other threads.
/// - On wasm, this is pre-filled synchronously.
#[cfg_attr(target_arch = "wasm32", allow(clippy::needless_pass_by_value))]
fn load(
    store_id: &re_log_types::StoreId,
    path: &std::path::Path,
    is_dir: bool,
    contents: Option<std::borrow::Cow<'_, [u8]>>,
) -> Result<std::sync::mpsc::Receiver<LoadedData>, DataLoaderError> {
    #[cfg(target_arch = "wasm32")]
    let no_external_loaders = true;
    #[cfg(not(target_arch = "wasm32"))]
    let no_external_loaders = EXTERNAL_LOADERS.is_empty();

    let extension = extension(path);
    let is_builtin = is_builtin(path, is_dir);

    // If there are no external loaders registered (which is always the case on wasm) and we don't
    // have a builtin loader for it, then we know for a fact that we won't be able to load it.
    if !is_builtin && no_external_loaders {
        return if extension.is_empty() {
            Err(anyhow::anyhow!("files without extensions (file.XXX) are not supported").into())
        } else {
            Err(anyhow::anyhow!(".{extension} files are not supported").into())
        };
    }

    // On native we run loaders in parallel so this needs to become static.
    // TODO: the copy is kinda dumb though
    #[cfg(not(target_arch = "wasm32"))]
    let contents: Option<std::borrow::Cow<'static, [u8]>> =
        contents.map(|contents| Cow::Owned(contents.into_owned()));

    let rx_loader = {
        let (tx_loader, rx_loader) = std::sync::mpsc::channel();

        let loaders = &BUILTIN_LOADERS;
        for loader in loaders.iter() {
            let mut contents = contents.clone(); // arc
            let loader = Arc::clone(loader);
            let store_id = store_id.clone();
            let tx_loader = tx_loader.clone();
            let path = path.to_owned();

            spawn(move || {
                if let Some(contents) = contents.take() {
                    if let Err(err) = loader.load_from_file_contents(
                        store_id,
                        path.clone(),
                        Cow::Borrowed(&contents),
                        tx_loader,
                    ) {
                        re_log::error!(?path, loader = loader.name(), %err, "failed to load data from file");
                    }
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Err(err) = loader.load_from_file(store_id, path.clone(), tx_loader) {
                        re_log::error!(?path, loader = loader.name(), %err, "failed to load data from files");
                    }
                }
            });
        }

        // Implicitly closing `tx_loader`!

        rx_loader
    };

    Ok(rx_loader)
}

/// Forwards the data in `rx_loader` to `tx`, taking care of necessary conversions, if any.
///
/// Runs asynchronously from another thread on native, synchronously on wasm.
fn send(
    store_id: &re_log_types::StoreId,
    rx_loader: std::sync::mpsc::Receiver<LoadedData>,
    tx: &Sender<LogMsg>,
) {
    spawn({
        let tx = tx.clone();
        let store_id = store_id.clone();
        move || {
            // ## Ignoring channel errors
            //
            // Not our problem whether or not the other end has hanged up, but we still want to
            // poll the channel in any case so as to make sure that the data producer
            // doesn't get stuck.
            for data in rx_loader {
                match data {
                    LoadedData::DataRow(row) => {
                        let mut table =
                            re_log_types::DataTable::from_rows(re_log_types::TableId::new(), [row]);
                        table.compute_all_size_bytes();

                        let arrow_msg = table.to_arrow_msg().unwrap(); // TODO

                        tx.send(LogMsg::ArrowMsg(store_id.clone(), arrow_msg)).ok();
                    }

                    LoadedData::ArrowMsg(msg) => {
                        tx.send(LogMsg::ArrowMsg(store_id.clone(), msg)).ok();
                    }

                    LoadedData::LogMsg(msg) => {
                        tx.send(msg).ok();
                    }
                }
            }

            tx.quit(None).ok();
        }
    });
}

// NOTE:
// - On native, we parallelize using `rayon`.
// - On wasm, we serialize everything, which works because the data-loading channels are unbounded.

#[cfg(not(target_arch = "wasm32"))]
fn spawn<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    rayon::spawn(f);
}

#[cfg(target_arch = "wasm32")]
fn spawn<F>(f: F)
where
    F: FnOnce(),
{
    f();
}
