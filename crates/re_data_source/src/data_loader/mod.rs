use std::sync::Arc;

use once_cell::sync::Lazy;

use re_log_types::{ArrowMsg, DataRow, LogMsg};

// ---

/// A [`DataLoader`] loads data from a file path and/or a file's contents.
///
/// Files can be loaded in 3 different ways:
/// - via the Rerun CLI (`rerun myfile.jpeg`),
/// - using drag-and-drop,
/// - using the open dialog in the Rerun Viewer.
///
/// All these file loading methods support loading a single file, many files at once, or even
/// folders.
/// ⚠ Drag-and-drop of folders does not yet work on the web version of Rerun Viewer ⚠
///
/// We only support loading files from the local filesystem at the moment, and consequently only
/// accept filepaths as input.
/// [There are plans to make this generic over any URI](https://github.com/rerun-io/rerun/issues/4525).
///
/// Rerun comes with a few [`DataLoader`]s by default:
/// - [`RrdLoader`] for [Rerun files].
/// - [`ArchetypeLoader`] for:
///     - [3D models]
///     - [Images]
///     - [Point clouds]
///     - [Text files]
/// - [`DirectoryLoader`] for recursively loading folders.
/// - `ExternalLoader`, which looks for user-defined data loaders in $PATH.
///
/// ## Execution
///
/// **All** registered [`DataLoader`]s get called when a user tries to open a file, unconditionally.
/// This gives [`DataLoader`]s maximum flexibility to decide what files they are interested in, as
/// opposed to e.g. only being able to look at files' extensions.
///
/// On native, [`DataLoader`]s are executed in parallel.
///
/// [Rerun files]: crate::SUPPORTED_RERUN_EXTENSIONS
/// [3D models]: crate::SUPPORTED_MESH_EXTENSIONS
/// [Images]: crate::SUPPORTED_IMAGE_EXTENSIONS
/// [Point clouds]: crate::SUPPORTED_POINT_CLOUD_EXTENSIONS
/// [Text files]: crate::SUPPORTED_TEXT_EXTENSIONS
//
// TODO(#4525): `DataLoader`s should support arbitrary URIs
// TODO(#4526): `DataLoader`s should be exposed to the SDKs
// TODO(#4527): Web Viewer `?url` parameter should accept anything our `DataLoader`s support
pub trait DataLoader: Send + Sync {
    /// Name of the [`DataLoader`].
    ///
    /// Doesn't need to be unique.
    fn name(&self) -> String;

    /// Loads data from a file on the local filesystem and sends it to `tx`.
    ///
    /// This is generally called when opening files with the Rerun CLI or via the open menu in the
    /// Rerun Viewer on native platforms.
    ///
    /// The passed-in `store_id` is a shared recording created by the file loading machinery:
    /// implementers can decide to use it or not (e.g. it might make sense to log all images with a
    /// similar name in a shared recording, while an rrd file is already its own recording).
    ///
    /// `path` isn't necessarily a _file_ path, but can be a directory as well: implementers are
    /// free to handle that however they decide.
    ///
    /// ## Error handling
    ///
    /// Most implementers of `load_from_path` are expected to be asynchronous in nature.
    ///
    /// Asynchronous implementers should make sure to fail early (and thus synchronously) when
    /// possible (e.g. didn't even manage to open the file).
    /// Otherwise, they should log errors that happen in an asynchronous context.
    ///
    /// If a [`DataLoader`] has no interest in the given file, it should successfully return
    /// without pushing any data into `tx`.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        store_id: re_log_types::StoreId,
        path: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError>;

    /// Loads data from in-memory file contents and sends it to `tx`.
    ///
    /// This is generally called when opening files via drag-and-drop or when using the web viewer.
    ///
    /// The passed-in `store_id` is a shared recording created by the file loading machinery:
    /// implementers can decide to use it or not (e.g. it might make sense to log all images with a
    /// similar name in a shared recording, while an rrd file is already its own recording).
    ///
    /// The `path` of the file is given for informational purposes (e.g. to extract the file's
    /// extension): implementers should _not_ try to read from disk as there is likely isn't a
    /// filesystem available to begin with.
    /// `path` is guaranteed to be a file path.
    ///
    /// When running on the web (wasm), `filepath` only contains the file name.
    ///
    /// ## Error handling
    ///
    /// Most implementers of `load_from_file_contents` are expected to be asynchronous in nature.
    ///
    /// Asynchronous implementers should make sure to fail early (and thus synchronously) when
    /// possible (e.g. didn't even manage to open the file).
    /// Otherwise, they should log errors that happen in an asynchronous context.
    ///
    /// If a [`DataLoader`] has no interest in the given file, it should successfully return
    /// without pushing any data into `tx`.
    fn load_from_file_contents(
        &self,
        store_id: re_log_types::StoreId,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
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

impl DataLoaderError {
    #[inline]
    pub fn is_path_not_found(&self) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            DataLoaderError::IO(err) => err.kind() == std::io::ErrorKind::NotFound,
            _ => false,
        }
    }
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
    #[inline]
    fn from(value: DataRow) -> Self {
        Self::DataRow(value)
    }
}

impl From<ArrowMsg> for LoadedData {
    #[inline]
    fn from(value: ArrowMsg) -> Self {
        LoadedData::ArrowMsg(value)
    }
}

impl From<LogMsg> for LoadedData {
    #[inline]
    fn from(value: LogMsg) -> Self {
        LoadedData::LogMsg(value)
    }
}

impl LoadedData {
    /// Pack the data into a [`LogMsg`].
    pub fn into_log_msg(
        self,
        store_id: &re_log_types::StoreId,
    ) -> Result<LogMsg, re_log_types::DataTableError> {
        match self {
            Self::DataRow(row) => {
                let mut table =
                    re_log_types::DataTable::from_rows(re_log_types::TableId::new(), [row]);
                table.compute_all_size_bytes();

                Ok(LogMsg::ArrowMsg(store_id.clone(), table.to_arrow_msg()?))
            }

            Self::ArrowMsg(msg) => Ok(LogMsg::ArrowMsg(store_id.clone(), msg)),

            Self::LogMsg(msg) => Ok(msg),
        }
    }
}

// ---

/// Keeps track of all builtin [`DataLoader`]s.
///
/// Lazy initialized the first time a file is opened.
static BUILTIN_LOADERS: Lazy<Vec<Arc<dyn DataLoader>>> = Lazy::new(|| {
    vec![
        Arc::new(RrdLoader) as Arc<dyn DataLoader>,
        Arc::new(ArchetypeLoader),
        Arc::new(DirectoryLoader),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(ExternalLoader),
    ]
});

/// Iterator over all registered [`DataLoader`]s.
#[inline]
pub fn iter_loaders() -> impl ExactSizeIterator<Item = Arc<dyn DataLoader>> {
    BUILTIN_LOADERS.clone().into_iter()
}

// ---

mod loader_archetype;
mod loader_directory;
mod loader_rrd;

#[cfg(not(target_arch = "wasm32"))]
mod loader_external;

pub use self::loader_archetype::ArchetypeLoader;
pub use self::loader_directory::DirectoryLoader;
pub use self::loader_rrd::RrdLoader;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use self::loader_external::EXTERNAL_LOADER_PATHS;
#[cfg(not(target_arch = "wasm32"))]
pub use self::loader_external::{
    iter_external_loaders, ExternalLoader, EXTERNAL_DATA_LOADER_PREFIX,
};
