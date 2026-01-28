//! Handles loading of Rerun data from file using data loader plugins.

use std::sync::{Arc, LazyLock};

use re_chunk::{Chunk, ChunkResult};
use re_log_types::{ArrowMsg, EntityPath, LogMsg, RecordingId, StoreId, TimePoint};

// ----------------------------------------------------------------------------

mod load_file;
mod loader_archetype;
mod loader_directory;
mod loader_rrd;
mod loader_urdf;

#[cfg(not(target_arch = "wasm32"))]
pub mod lerobot;

// This loader currently only works when loading the entire dataset directory, and we cannot do that on web yet.
#[cfg(not(target_arch = "wasm32"))]
pub mod loader_lerobot;

// This loader currently uses native-only features under the hood, and we cannot do that on web yet.
pub mod loader_mcap;

#[cfg(not(target_arch = "wasm32"))]
mod loader_external;

pub use self::load_file::load_from_file_contents;
pub use self::loader_archetype::ArchetypeLoader;
pub use self::loader_directory::DirectoryLoader;
pub use self::loader_mcap::McapLoader;
pub use self::loader_rrd::RrdLoader;
pub use self::loader_urdf::{UrdfDataLoader, UrdfTree};
#[cfg(not(target_arch = "wasm32"))]
pub use self::{
    load_file::load_from_path,
    loader_external::{
        EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE, EXTERNAL_DATA_LOADER_PREFIX, ExternalLoader,
        iter_external_loaders,
    },
    loader_lerobot::LeRobotDatasetLoader,
};

pub mod external {
    pub use urdf_rs;
}

// ----------------------------------------------------------------------------

/// Recommended settings for the [`DataLoader`].
///
/// The loader is free to ignore some or all of these.
///
/// External [`DataLoader`]s will be passed the following CLI parameters:
/// * `--application-id <application_id>`
/// * `--opened-application-id <opened_application_id>` (if set)
/// * `--recording-id <store_id>`
/// * `--opened-recording-id <opened_store_id>` (if set)
/// * `--entity-path-prefix <entity_path_prefix>` (if set)
/// * `--static` (if `timepoint` is set to the timeless timepoint)
/// * `--timeless` \[deprecated\] (if `timepoint` is set to the timeless timepoint)
/// * `--time_sequence <timeline1>=<seq1> <timeline2>=<seq2> ...` (if `timepoint` contains sequence data)
/// * `--time_duration_nanos <timeline1>=<duration1> <timeline2>=<duration2> ...` (if `timepoint` contains duration data) in nanos
/// * `--time_timestamp_nanos <timeline1>=<timestamp1> <timeline2>=<timestamp2> ...` (if `timepoint` contains timestamp data) in nanos since epoch
#[derive(Debug, Clone)]
pub struct DataLoaderSettings {
    /// The recommended [`re_log_types::ApplicationId`] to log the data to, based on the surrounding context.
    pub application_id: Option<re_log_types::ApplicationId>,

    /// The recommended recording id to log the data to, based on the surrounding context.
    ///
    /// Log data to this recording if you want it to appear in a new recording shared by all
    /// data-loaders for the current loading session.
    pub recording_id: RecordingId,

    /// The [`re_log_types::StoreId`] that is currently opened in the viewer, if any.
    pub opened_store_id: Option<StoreId>,

    /// Whether `SetStoreInfo`s should be sent, regardless of the surrounding context.
    ///
    /// Only useful when creating a recording just-in-time directly in the viewer (which is what
    /// happens when importing things into the welcome screen).
    pub force_store_info: bool,

    /// What should the logged entity paths be prefixed with?
    pub entity_path_prefix: Option<EntityPath>,

    /// At what time(s) should the data be logged to?
    pub timepoint: Option<TimePoint>,
}

impl DataLoaderSettings {
    #[inline]
    pub fn recommended(recording_id: impl Into<RecordingId>) -> Self {
        Self {
            application_id: Default::default(),
            recording_id: recording_id.into(),
            opened_store_id: Default::default(),
            force_store_info: false,
            entity_path_prefix: Default::default(),
            timepoint: Default::default(),
        }
    }

    /// Returns the recommended [`re_log_types::StoreId`] to log the data to.
    pub fn recommended_store_id(&self) -> StoreId {
        StoreId::recording(
            self.application_id
                .clone()
                .unwrap_or_else(re_log_types::ApplicationId::random),
            self.recording_id.clone(),
        )
    }

    /// Returns the currently opened [`re_log_types::StoreId`] if any. Otherwise, returns the
    /// recommended store id.
    pub fn opened_store_id_or_recommended(&self) -> StoreId {
        self.opened_store_id
            .clone()
            .unwrap_or_else(|| self.recommended_store_id())
    }

    /// Generates CLI flags from these settings, for external data loaders.
    pub fn to_cli_args(&self) -> Vec<String> {
        let Self {
            application_id,
            recording_id,
            opened_store_id,
            force_store_info: _,
            entity_path_prefix,
            timepoint,
        } = self;

        let mut args = Vec::new();

        if let Some(application_id) = application_id {
            args.extend(["--application-id".to_owned(), format!("{application_id}")]);
        }
        args.extend(["--recording-id".to_owned(), format!("{recording_id}")]);

        if let Some(opened_store_id) = opened_store_id {
            args.extend([
                "--opened-application-id".to_owned(),
                format!("{}", opened_store_id.application_id()),
            ]);

            args.extend([
                "--opened-recording-id".to_owned(),
                format!("{}", opened_store_id.recording_id()),
            ]);
        }

        if let Some(entity_path_prefix) = entity_path_prefix {
            args.extend([
                "--entity-path-prefix".to_owned(),
                format!("{entity_path_prefix}"),
            ]);
        }

        if let Some(timepoint) = timepoint {
            if timepoint.is_static() {
                args.push("--timeless".to_owned()); // for backwards compatibility
                args.push("--static".to_owned());
            }

            for (timeline, cell) in timepoint.iter() {
                match cell.typ() {
                    re_log_types::TimeType::Sequence => {
                        args.extend([
                            "--time_sequence".to_owned(),
                            format!("{timeline}={}", cell.value),
                        ]);

                        // for backwards compatibility:
                        args.extend([
                            "--sequence".to_owned(),
                            format!("{timeline}={}", cell.value),
                        ]);
                    }
                    re_log_types::TimeType::DurationNs => {
                        args.extend([
                            "--time_duration_nanos".to_owned(),
                            format!("{timeline}={}", cell.value),
                        ]);

                        // for backwards compatibility:
                        args.extend(["--time".to_owned(), format!("{timeline}={}", cell.value)]);
                    }
                    re_log_types::TimeType::TimestampNs => {
                        args.extend([
                            "--time_duration_nanos".to_owned(),
                            format!("{timeline}={}", cell.value),
                        ]);

                        // for backwards compatibility:
                        args.extend([
                            "--sequence".to_owned(),
                            format!("{timeline}={}", cell.value),
                        ]);
                    }
                }
            }
        }

        args
    }
}

pub type DataLoaderName = String;

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
/// Checkout our [guide](https://www.rerun.io/docs/reference/data-loaders/overview).
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
// TODO(#4527): Web Viewer `?url` parameter should accept anything our `DataLoader`s support
pub trait DataLoader: Send + Sync {
    /// Name of the [`DataLoader`].
    ///
    /// Should be globally unique.
    fn name(&self) -> DataLoaderName;

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
        tx: crossbeam::channel::Sender<LoadedData>,
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
        tx: crossbeam::channel::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError>;
}

/// Errors that might happen when loading data through a [`DataLoader`].
#[derive(thiserror::Error, Debug)]
pub enum DataLoaderError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    Decode(#[from] re_log_encoding::DecodeError),

    #[error("No data-loader support for {0:?}")]
    Incompatible(std::path::PathBuf),

    #[error(transparent)]
    Mcap(#[from] ::mcap::McapError),

    #[error("{}", re_error::format(.0))]
    Other(#[from] anyhow::Error),
}

impl DataLoaderError {
    #[inline]
    pub fn is_path_not_found(&self) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::IO(err) => err.kind() == std::io::ErrorKind::NotFound,
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
    Chunk(DataLoaderName, re_log_types::StoreId, Chunk),
    ArrowMsg(DataLoaderName, re_log_types::StoreId, ArrowMsg),
    LogMsg(DataLoaderName, LogMsg),
}

impl LoadedData {
    /// Returns the name of the [`DataLoader`] that generated this data.
    #[inline]
    pub fn data_loader_name(&self) -> &DataLoaderName {
        match self {
            Self::Chunk(name, ..) | Self::ArrowMsg(name, ..) | Self::LogMsg(name, ..) => name,
        }
    }

    /// Pack the data into a [`LogMsg`].
    #[inline]
    pub fn into_log_msg(self) -> ChunkResult<LogMsg> {
        match self {
            Self::Chunk(_name, store_id, chunk) => {
                Ok(LogMsg::ArrowMsg(store_id, chunk.to_arrow_msg()?))
            }

            Self::ArrowMsg(_name, store_id, msg) => Ok(LogMsg::ArrowMsg(store_id, msg)),

            Self::LogMsg(_name, msg) => Ok(msg),
        }
    }
}

// ----------------------------------------------------------------------------

/// Keeps track of all builtin [`DataLoader`]s.
///
/// Lazy initialized the first time a file is opened.
static BUILTIN_LOADERS: LazyLock<Vec<Arc<dyn DataLoader>>> = LazyLock::new(|| {
    vec![
        Arc::new(RrdLoader) as Arc<dyn DataLoader>,
        Arc::new(ArchetypeLoader),
        Arc::new(DirectoryLoader),
        Arc::new(McapLoader::default()),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(LeRobotDatasetLoader),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(ExternalLoader),
        Arc::new(UrdfDataLoader),
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
static CUSTOM_LOADERS: LazyLock<parking_lot::RwLock<Vec<Arc<dyn DataLoader>>>> =
    LazyLock::new(parking_lot::RwLock::default);

/// Register a custom [`DataLoader`].
///
/// Any time the Rerun Viewer opens a file or directory, this custom loader will be notified.
/// Refer to [`DataLoader`]'s documentation for more information.
#[inline]
pub fn register_custom_data_loader(loader: impl DataLoader + 'static) {
    CUSTOM_LOADERS.write().push(Arc::new(loader));
}

// ----------------------------------------------------------------------------

/// Empty string if no extension.
#[inline]
pub(crate) fn extension(path: &std::path::Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string()
}

// ----------------------------------------------------------------------------

// …given that all feature flags are turned on for the `image` crate.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "dds", "exr", "farbfeld", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "pam",
    "pbm", "pgm", "png", "ppm", "tga", "tif", "tiff", "webp",
];

pub const SUPPORTED_DEPTH_IMAGE_EXTENSIONS: &[&str] = &["rvl", "png"];

pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &["mp4"];

pub const SUPPORTED_MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj", "stl", "dae"];

// TODO(#4532): `.ply` data loader should support 2D point cloud & meshes
pub const SUPPORTED_POINT_CLOUD_EXTENSIONS: &[&str] = &["ply"];

pub const SUPPORTED_RERUN_EXTENSIONS: &[&str] = &["rbl", "rrd"];

/// 3rd party formats with built-in support.
pub const SUPPORTED_THIRD_PARTY_FORMATS: &[&str] = &["mcap", "urdf"];

// TODO(#4555): Add catch-all builtin `DataLoader` for text files
pub const SUPPORTED_TEXT_EXTENSIONS: &[&str] = &["txt", "md"];

/// All file extension supported by our builtin [`DataLoader`]s.
pub fn supported_extensions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_RERUN_EXTENSIONS
        .iter()
        .chain(SUPPORTED_THIRD_PARTY_FORMATS)
        .chain(SUPPORTED_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_DEPTH_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_VIDEO_EXTENSIONS)
        .chain(SUPPORTED_MESH_EXTENSIONS)
        .chain(SUPPORTED_POINT_CLOUD_EXTENSIONS)
        .chain(SUPPORTED_TEXT_EXTENSIONS)
        .copied()
}

/// Is this a supported file extension by any of our builtin [`DataLoader`]s?
pub fn is_supported_file_extension(extension: &str) -> bool {
    debug_assert!(
        !extension.starts_with('.'),
        "Expected extension without period, but got {extension:?}"
    );
    let extension = extension.to_lowercase();
    supported_extensions().any(|ext| ext == extension)
}

#[test]
fn test_supported_extensions() {
    assert!(is_supported_file_extension("rrd"));
    assert!(is_supported_file_extension("mcap"));
    assert!(is_supported_file_extension("png"));
    assert!(is_supported_file_extension("urdf"));
}
