use std::sync::Arc;

use once_cell::sync::Lazy;

use re_log_types::{ArrowMsg, DataRow, EntityPath, LogMsg, TimePoint};

// ---

/// Recommended settings for the [`DataLoader`].
///
/// The loader is free to ignore some or all of these.
///
/// External [`DataLoader`]s will be passed the following CLI parameters:
/// * `--recording-id <store_id>`
/// * `--opened-recording-id <opened_store_id>` (if set)
/// * `--entity-path-prefix <entity_path_prefix>` (if set)
/// * `--timeless` (if `timepoint` is set to the timeless timepoint)
/// * `--time <timeline1>=<time1> <timeline2>=<time2> ...` (if `timepoint` contains temporal data)
/// * `--sequence <timeline1>=<seq1> <timeline2>=<seq2> ...` (if `timepoint` contains sequence data)
#[derive(Debug, Clone)]
pub struct DataLoaderSettings {
    /// The recommended [`re_log_types::StoreId`] to log the data to, based on the surrounding context.
    pub store_id: re_log_types::StoreId,

    /// The [`re_log_types::StoreId`] that is currently opened in the viewer, if any.
    ///
    /// Log data to this recording if you want it to appear in a new recording shared by all
    /// data-loaders for the current loading session.
    //
    // TODO(#5350): actually support this
    pub opened_store_id: Option<re_log_types::StoreId>,

    /// What should the entity paths be prefixed with?
    pub entity_path_prefix: Option<EntityPath>,

    /// At what time(s) should the data be logged to?
    pub timepoint: Option<TimePoint>,
}

impl DataLoaderSettings {
    #[inline]
    pub fn recommended(store_id: impl Into<re_log_types::StoreId>) -> Self {
        Self {
            store_id: store_id.into(),
            opened_store_id: Default::default(),
            entity_path_prefix: Default::default(),
            timepoint: Default::default(),
        }
    }

    /// Generates CLI flags from these settings, for external data loaders.
    pub fn to_cli_args(&self) -> Vec<String> {
        let Self {
            store_id,
            opened_store_id,
            entity_path_prefix,
            timepoint,
        } = self;

        let mut args = Vec::new();

        args.extend(["--recording-id".to_owned(), format!("{store_id}")]);

        if let Some(opened_store_id) = opened_store_id {
            args.extend([
                "--opened-recording-id".to_owned(),
                format!("{opened_store_id}"),
            ]);
        }

        if let Some(entity_path_prefix) = entity_path_prefix {
            args.extend([
                "--entity-path-prefix".to_owned(),
                format!("{entity_path_prefix}"),
            ]);
        }

        if let Some(timepoint) = timepoint {
            if timepoint.is_timeless() {
                args.push("--timeless".to_owned());
            }

            for (timeline, time) in timepoint.iter() {
                match timeline.typ() {
                    re_log_types::TimeType::Time => {
                        args.extend([
                            "--time".to_owned(),
                            format!("{}={}", timeline.name(), time.as_i64()),
                        ]);
                    }
                    re_log_types::TimeType::Sequence => {
                        args.extend([
                            "--sequence".to_owned(),
                            format!("{}={}", timeline.name(), time.as_i64()),
                        ]);
                    }
                }
            }
        }

        args
    }
}

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
/// - [`ExternalLoader`], which looks for user-defined data loaders in $PATH.
///
/// ## Registering custom loaders
///
/// Checkout our [guide](https://www.rerun.io/docs/howto/open-any-file).
///
/// ## Execution
///
/// **All** known [`DataLoader`]s get called when a user tries to open a file, unconditionally.
/// This gives [`DataLoader`]s maximum flexibility to decide what files they are interested in, as
/// opposed to e.g. only being able to look at files' extensions.
///
/// If a [`DataLoader`] has no interest in the given file, it should fail as soon as possible
/// with a [`DataLoaderError::Incompatible`] error.
///
/// Iff all [`DataLoader`]s (including custom and external ones) return with a [`DataLoaderError::Incompatible`]
/// error, the Viewer will show an error message to the user indicating that the file type is not
/// supported.
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
    /// If a [`DataLoader`] has no interest in the given file, it should fail as soon as possible
    /// with a [`DataLoaderError::Incompatible`] error.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &DataLoaderSettings,
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
    /// If a [`DataLoader`] has no interest in the given file, it should fail as soon as possible
    /// with a [`DataLoaderError::Incompatible`] error.
    fn load_from_file_contents(
        &self,
        settings: &DataLoaderSettings,
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

    #[error("No data-loader support for {0:?}")]
    Incompatible(std::path::PathBuf),

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

    #[inline]
    pub fn is_incompatible(&self) -> bool {
        matches!(self, Self::Incompatible { .. })
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
pub fn iter_loaders() -> impl Iterator<Item = Arc<dyn DataLoader>> {
    BUILTIN_LOADERS
        .clone()
        .into_iter()
        .chain(CUSTOM_LOADERS.read().clone())
}

/// Keeps track of all custom [`DataLoader`]s.
///
/// Use [`register_custom_data_loader`] to add new loaders.
static CUSTOM_LOADERS: Lazy<parking_lot::RwLock<Vec<Arc<dyn DataLoader>>>> =
    Lazy::new(parking_lot::RwLock::default);

/// Register a custom [`DataLoader`].
///
/// Any time the Rerun Viewer opens a file or directory, this custom loader will be notified.
/// Refer to [`DataLoader`]'s documentation for more information.
#[inline]
pub fn register_custom_data_loader(loader: impl DataLoader + 'static) {
    CUSTOM_LOADERS.write().push(Arc::new(loader));
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
pub use self::loader_external::{
    iter_external_loaders, ExternalLoader, EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE,
    EXTERNAL_DATA_LOADER_PREFIX,
};
