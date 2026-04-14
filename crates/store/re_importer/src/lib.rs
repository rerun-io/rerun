//! Handles importing of Rerun data from file using importer plugins.

use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

use re_chunk::{Chunk, ChunkResult};
use re_log_types::{ArrowMsg, EntityPath, LogMsg, RecordingId, StoreId, TimePoint};

// ----------------------------------------------------------------------------

mod import_file;
mod importer_archetype;
mod importer_directory;
mod importer_rrd;
mod importer_urdf;

#[cfg(not(target_arch = "wasm32"))]
pub mod lerobot;

// This importer currently only works when loading the entire dataset directory, and we cannot do that on web yet.
#[cfg(not(target_arch = "wasm32"))]
pub mod importer_lerobot;

// This importer currently uses native-only features under the hood, and we cannot do that on web yet.
pub mod importer_mcap;

#[cfg(not(target_arch = "wasm32"))]
mod importer_external;
#[cfg(not(target_arch = "wasm32"))]
pub mod importer_parquet;

pub use self::import_file::{import_from_file_contents, prepare_store_info};
pub use self::importer_archetype::ArchetypeImporter;
pub use self::importer_directory::DirectoryImporter;
pub use self::importer_mcap::McapImporter;
pub use self::importer_rrd::RrdImporter;
pub use self::importer_urdf::{UrdfImporter, UrdfTree, joint_transform as urdf_joint_transform};
#[cfg(not(target_arch = "wasm32"))]
pub use self::{
    import_file::import_from_path,
    importer_external::{
        EXTERNAL_IMPORTER_INCOMPATIBLE_EXIT_CODE, EXTERNAL_IMPORTER_PREFIX, ExternalImporter,
        iter_external_importers,
    },
    importer_lerobot::LeRobotDatasetImporter,
    importer_parquet::ParquetImporter,
};

pub mod external {
    pub use urdf_rs;
}

// ----------------------------------------------------------------------------

/// The identifier used to enable or disable Foxglove lenses when loading MCAP files.
pub const FOXGLOVE_LENSES_IDENTIFIER: &str = "foxglove";

/// The identifier used to enable or disable URDF extraction from MCAP `robot_description` topics.
pub const URDF_DECODER_IDENTIFIER: &str = "urdf";

/// All decoder-like identifiers supported by [`McapImporter`].
///
/// This merges the built-in MCAP decoders from [`re_mcap`] and the semantic interpretation (e.g. lenses) that are in this crate.
pub fn supported_mcap_decoder_identifiers(
    raw_fallback_enabled: bool,
) -> Vec<re_mcap::DecoderIdentifier> {
    let mut identifiers = re_mcap::DecoderRegistry::all_builtin(raw_fallback_enabled)
        .all_identifiers()
        .into_iter()
        .map(re_mcap::DecoderIdentifier::from)
        .collect::<BTreeSet<_>>();

    identifiers.extend([
        re_mcap::DecoderIdentifier::from(FOXGLOVE_LENSES_IDENTIFIER),
        re_mcap::DecoderIdentifier::from(URDF_DECODER_IDENTIFIER),
    ]);

    identifiers.into_iter().collect()
}

// ----------------------------------------------------------------------------

/// Recommended settings for the [`Importer`].
///
/// The importer is free to ignore some or all of these.
///
/// External [`Importer`]s will be passed the following CLI parameters:
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
pub struct ImporterSettings {
    /// The recommended [`re_log_types::ApplicationId`] to log the data to, based on the surrounding context.
    pub application_id: Option<re_log_types::ApplicationId>,

    /// The recommended recording id to log the data to, based on the surrounding context.
    ///
    /// Log data to this recording if you want it to appear in a new recording shared by all
    /// importers for the current loading session.
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

    /// If `true`, keep reading `.rrd` files past EOF, tailing new data as it arrives.
    ///
    /// Defaults to `false`.
    pub follow: bool,

    /// If set, an offset in nanoseconds to add to all `TimestampNs` time columns.
    pub timestamp_offset_ns: Option<i64>,

    /// The timeline type to use for timestamp timelines.
    ///
    /// Defaults to [`re_log_types::TimeType::TimestampNs`].
    /// When set to [`re_log_types::TimeType::DurationNs`], all timestamp timelines
    /// will be created as duration timelines instead.
    pub timeline_type: re_log_types::TimeType,
}

impl ImporterSettings {
    #[inline]
    pub fn recommended(recording_id: impl Into<RecordingId>) -> Self {
        Self {
            recording_id: recording_id.into(),
            application_id: None,
            opened_store_id: None,
            force_store_info: false,
            entity_path_prefix: None,
            timepoint: None,
            follow: false,
            timestamp_offset_ns: None,
            timeline_type: re_log_types::TimeType::TimestampNs,
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

    /// Generates CLI flags from these settings, for external importers.
    pub fn to_cli_args(&self) -> Vec<String> {
        let Self {
            application_id,
            recording_id,
            opened_store_id,
            force_store_info: _,
            entity_path_prefix,
            timepoint,
            follow: _,
            timestamp_offset_ns: _,
            timeline_type: _,
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

pub type ImporterName = String;

/// An [`Importer`] imports data from a file path and/or a file's contents.
///
/// Files can be imported in 3 different ways:
/// - via the Rerun CLI (`rerun myfile.jpeg`),
/// - using drag-and-drop,
/// - using the open dialog in the Rerun Viewer.
///
/// All these file importing methods support importing a single file, many files at once, or even
/// folders.
/// ⚠ Drag-and-drop of folders does not yet work on the web version of Rerun Viewer ⚠
///
/// We only support importing files from the local filesystem at the moment, and consequently only
/// accept filepaths as input.
/// [There are plans to make this generic over any URI](https://github.com/rerun-io/rerun/issues/4525).
///
/// Rerun comes with a few [`Importer`]s by default:
/// - [`RrdImporter`] for [Rerun files].
/// - [`ArchetypeImporter`] for:
///     - [3D models]
///     - [Images]
///     - [Point clouds]
///     - [Text files]
/// - [`DirectoryImporter`] for recursively importing folders.
/// - [`ExternalImporter`], which looks for user-defined importers in $PATH.
///
/// ## Registering custom importers
///
/// Checkout our [guide](https://www.rerun.io/docs/concepts/logging-and-ingestion/importers/overview?speculative-link).
///
/// ## Execution
///
/// **All** known [`Importer`]s get called when a user tries to open a file, unconditionally.
/// This gives [`Importer`]s maximum flexibility to decide what files they are interested in, as
/// opposed to e.g. only being able to look at files' extensions.
///
/// If an [`Importer`] has no interest in the given file, it should fail as soon as possible
/// with a [`ImporterError::Incompatible`] error.
///
/// Iff all [`Importer`]s (including custom and external ones) return with a [`ImporterError::Incompatible`]
/// error, the Viewer will show an error message to the user indicating that the file type is not
/// supported.
///
/// On native, [`Importer`]s are executed in parallel.
///
/// [Rerun files]: crate::SUPPORTED_RERUN_EXTENSIONS
/// [3D models]: crate::SUPPORTED_MESH_EXTENSIONS
/// [Images]: crate::SUPPORTED_IMAGE_EXTENSIONS
/// [Point clouds]: crate::SUPPORTED_POINT_CLOUD_EXTENSIONS
/// [Text files]: crate::SUPPORTED_TEXT_EXTENSIONS
//
// TODO(#4525): `Importer`s should support arbitrary URIs
// TODO(#4527): Web Viewer `?url` parameter should accept anything our `Importer`s support
pub trait Importer: Send + Sync {
    /// Name of the [`Importer`].
    ///
    /// Should be globally unique.
    fn name(&self) -> ImporterName;

    /// Imports data from a file on the local filesystem and sends it to `tx`.
    ///
    /// This is generally called when opening files with the Rerun CLI or via the open menu in the
    /// Rerun Viewer on native platforms.
    ///
    /// The passed-in `store_id` is a shared recording created by the file importing machinery:
    /// implementers can decide to use it or not (e.g. it might make sense to log all images with a
    /// similar name in a shared recording, while an rrd file is already its own recording).
    ///
    /// `path` isn't necessarily a _file_ path, but can be a directory as well: implementers are
    /// free to handle that however they decide.
    ///
    /// ## Error handling
    ///
    /// Most implementers of `import_from_path` are expected to be asynchronous in nature.
    ///
    /// Asynchronous implementers should make sure to fail early (and thus synchronously) when
    /// possible (e.g. didn't even manage to open the file).
    /// Otherwise, they should log errors that happen in an asynchronous context.
    ///
    /// If an [`Importer`] has no interest in the given file, it should fail as soon as possible
    /// with a [`ImporterError::Incompatible`] error.
    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &ImporterSettings,
        path: std::path::PathBuf,
        tx: crossbeam::channel::Sender<ImportedData>,
    ) -> Result<(), ImporterError>;

    /// Imports data from in-memory file contents and sends it to `tx`.
    ///
    /// This is generally called when opening files via drag-and-drop or when using the web viewer.
    ///
    /// The passed-in `store_id` is a shared recording created by the file importing machinery:
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
    /// Most implementers of `import_from_file_contents` are expected to be asynchronous in nature.
    ///
    /// Asynchronous implementers should make sure to fail early (and thus synchronously) when
    /// possible (e.g. didn't even manage to open the file).
    /// Otherwise, they should log errors that happen in an asynchronous context.
    ///
    /// If an [`Importer`] has no interest in the given file, it should fail as soon as possible
    /// with a [`ImporterError::Incompatible`] error.
    fn import_from_file_contents(
        &self,
        settings: &ImporterSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: crossbeam::channel::Sender<ImportedData>,
    ) -> Result<(), ImporterError>;
}

/// Errors that might happen when importing data through an [`Importer`].
#[derive(thiserror::Error, Debug)]
pub enum ImporterError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    Decode(#[from] re_log_encoding::DecodeError),

    #[error("No importer support for {0:?}")]
    Incompatible(std::path::PathBuf),

    #[error(transparent)]
    Mcap(#[from] ::mcap::McapError),

    #[error("{}", re_error::format(.0))]
    Other(#[from] anyhow::Error),
}

impl ImporterError {
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

/// What [`Importer`]s produce.
///
/// This makes it trivial for [`Importer`]s to build the data in whatever form is
/// most convenient for them, whether it is raw components, arrow chunks or even
/// full-on [`LogMsg`]s.
#[derive(Debug)]
pub enum ImportedData {
    Chunk(ImporterName, re_log_types::StoreId, Chunk),
    ArrowMsg(ImporterName, re_log_types::StoreId, ArrowMsg),
    LogMsg(ImporterName, LogMsg),
}

impl ImportedData {
    /// Returns the name of the [`Importer`] that generated this data.
    #[inline]
    pub fn importer_name(&self) -> &ImporterName {
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

    /// Convert the data into a [`Chunk`], ignoring all non-chunk-related things.
    pub fn into_chunk(self) -> Option<Chunk> {
        match self {
            Self::Chunk(_name, _store_id, chunk) => Some(chunk),
            Self::ArrowMsg(_name, _store_id, arrow_msg) => Chunk::from_arrow_msg(&arrow_msg).ok(),
            Self::LogMsg(_name, msg) => match msg {
                LogMsg::ArrowMsg(_store_id, arrow_msg) => Chunk::from_arrow_msg(&arrow_msg).ok(),
                LogMsg::SetStoreInfo { .. } | LogMsg::BlueprintActivationCommand { .. } => None,
            },
        }
    }
}

// ----------------------------------------------------------------------------

/// Keeps track of all builtin [`Importer`]s.
///
/// Lazy initialized the first time a file is opened.
static BUILTIN_IMPORTERS: LazyLock<Vec<Arc<dyn Importer>>> = LazyLock::new(|| {
    vec![
        Arc::new(RrdImporter) as Arc<dyn Importer>,
        Arc::new(ArchetypeImporter),
        Arc::new(DirectoryImporter),
        Arc::new(McapImporter::default()),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(ParquetImporter::default()),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(LeRobotDatasetImporter),
        #[cfg(not(target_arch = "wasm32"))]
        Arc::new(ExternalImporter),
        Arc::new(UrdfImporter),
    ]
});

/// Iterator over all registered [`Importer`]s.
#[inline]
pub fn iter_importers() -> impl Iterator<Item = Arc<dyn Importer>> {
    BUILTIN_IMPORTERS
        .clone()
        .into_iter()
        .chain(CUSTOM_IMPORTERS.read().clone())
}

/// Keeps track of all custom [`Importer`]s.
///
/// Use [`register_custom_importer`] to add new importers.
static CUSTOM_IMPORTERS: LazyLock<parking_lot::RwLock<Vec<Arc<dyn Importer>>>> =
    LazyLock::new(parking_lot::RwLock::default);

/// Register a custom [`Importer`].
///
/// Any time the Rerun Viewer opens a file or directory, this custom importer will be notified.
/// Refer to [`Importer`]'s documentation for more information.
#[inline]
pub fn register_custom_importer(importer: impl Importer + 'static) {
    CUSTOM_IMPORTERS.write().push(Arc::new(importer));
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

// ...given that all feature flags are turned on for the `image` crate.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "dds", "exr", "farbfeld", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "pam",
    "pbm", "pgm", "png", "ppm", "tga", "tif", "tiff", "webp",
];

pub const SUPPORTED_DEPTH_IMAGE_EXTENSIONS: &[&str] = &["rvl", "png"];

pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &["mp4"];

pub const SUPPORTED_MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj", "stl", "dae"];

// TODO(#4532): `.ply` importer should support 2D point cloud & meshes
pub const SUPPORTED_POINT_CLOUD_EXTENSIONS: &[&str] = &["ply"];

pub const SUPPORTED_RERUN_EXTENSIONS: &[&str] = &["rbl", "rrd"];

/// 3rd party formats with built-in support.
pub const SUPPORTED_THIRD_PARTY_FORMATS: &[&str] = &["mcap", "urdf"];

pub const SUPPORTED_PARQUET_EXTENSIONS: &[&str] = &["parquet"];

// TODO(#4555): Add catch-all builtin `Importer` for text files
pub const SUPPORTED_TEXT_EXTENSIONS: &[&str] = &["txt", "md"];

/// All file extension supported by our builtin [`Importer`]s.
pub fn supported_extensions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_RERUN_EXTENSIONS
        .iter()
        .chain(SUPPORTED_THIRD_PARTY_FORMATS)
        .chain(SUPPORTED_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_DEPTH_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_VIDEO_EXTENSIONS)
        .chain(SUPPORTED_MESH_EXTENSIONS)
        .chain(SUPPORTED_POINT_CLOUD_EXTENSIONS)
        .chain(SUPPORTED_PARQUET_EXTENSIONS)
        .chain(SUPPORTED_TEXT_EXTENSIONS)
        .copied()
}

/// Is this a supported file extension by any of our builtin [`Importer`]s?
pub fn is_supported_file_extension(extension: &str) -> bool {
    re_log::debug_assert!(
        !extension.starts_with('.'),
        "Expected extension without period, but got {extension:?}"
    );
    let extension = extension.to_lowercase();
    supported_extensions().any(|ext| ext == extension)
}

/// Detect the file format from the first bytes of a file (magic bytes).
///
/// Returns the file extension (e.g., `"rrd"`, `"mcap"`, `"png"`) if the format is recognized.
///
/// Delegates to [`re_sdk_types::components::MediaType::guess_from_data`] which handles
/// Robotics-specific formats (RRD, MCAP, PLY) and standard formats (PNG, JPEG, GLB, MP4, etc.).
pub fn detect_format_from_bytes(bytes: &[u8]) -> Option<String> {
    let media_type = re_sdk_types::components::MediaType::guess_from_data(bytes)?;
    media_type.file_extension().map(|e| e.to_owned())
}

/// Map a MIME content type to a file extension.
///
/// Returns `None` for types that are too generic to be useful (e.g. `application/octet-stream`)
/// or for unrecognized types.
///
/// Delegates to [`re_sdk_types::components::MediaType::file_extension`].
pub fn content_type_to_extension(content_type: &str) -> Option<String> {
    // Take just the MIME type, ignoring parameters like charset
    let mime = content_type.split(';').next()?.trim();

    // Skip overly generic types
    if mime == "application/octet-stream" {
        return None;
    }

    let media_type = re_sdk_types::components::MediaType(mime.to_owned().into());
    media_type.file_extension().map(|e| e.to_owned())
}

#[test]
fn test_supported_extensions() {
    assert!(is_supported_file_extension("rrd"));
    assert!(is_supported_file_extension("mcap"));
    assert!(is_supported_file_extension("png"));
    assert!(is_supported_file_extension("urdf"));
}

#[test]
fn test_supported_mcap_decoder_identifiers() {
    let identifiers = supported_mcap_decoder_identifiers(true);
    let as_strings = identifiers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    // Check that expected identifiers are present.
    assert!(as_strings.contains(&FOXGLOVE_LENSES_IDENTIFIER.to_owned()));
    assert!(as_strings.contains(&URDF_DECODER_IDENTIFIER.to_owned()));
    assert!(as_strings.contains(&"raw".to_owned()));
    assert!(as_strings.contains(&"protobuf".to_owned()));
    assert!(as_strings.contains(&"ros2msg".to_owned()));

    // Check that all identifiers are unique.
    let unique = as_strings.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(as_strings.len(), unique.len());
}

#[test]
fn test_detect_format_from_bytes() {
    assert_eq!(
        detect_format_from_bytes(b"RRF2xxxxx").as_deref(),
        Some("rrd")
    );
    assert_eq!(
        detect_format_from_bytes(b"RRF0xxxxx").as_deref(),
        Some("rrd")
    );
    assert_eq!(
        detect_format_from_bytes(&[0x89, 0x4D, 0x43, 0x41, 0x50, 0x30, 0x0D, 0x0A]).as_deref(),
        Some("mcap")
    );
    assert_eq!(
        detect_format_from_bytes(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).as_deref(),
        Some("png")
    );
    assert_eq!(
        detect_format_from_bytes(&[0xFF, 0xD8, 0xFF, 0xE0]).as_deref(),
        Some("jpg")
    );
    assert_eq!(
        detect_format_from_bytes(b"glTFxxxx").as_deref(),
        Some("glb")
    );
    assert_eq!(
        detect_format_from_bytes(b"ply\nxxx").as_deref(),
        Some("ply")
    );
    assert_eq!(detect_format_from_bytes(b"unknown").as_deref(), None);
    assert_eq!(detect_format_from_bytes(b"").as_deref(), None);
}

#[test]
fn test_content_type_to_extension() {
    assert_eq!(
        content_type_to_extension("image/png").as_deref(),
        Some("png")
    );
    assert_eq!(
        content_type_to_extension("image/png; charset=utf-8").as_deref(),
        Some("png")
    );
    assert_eq!(
        content_type_to_extension("image/jpeg").as_deref(),
        Some("jpg")
    );
    assert_eq!(
        content_type_to_extension("video/mp4").as_deref(),
        Some("mp4")
    );
    assert_eq!(
        content_type_to_extension("model/gltf-binary").as_deref(),
        Some("glb")
    );
    assert_eq!(
        content_type_to_extension("application/x-rerun").as_deref(),
        Some("rrd")
    );
    assert_eq!(
        content_type_to_extension("application/octet-stream").as_deref(),
        None
    );
}
