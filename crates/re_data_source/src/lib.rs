//! Handles different ways of loading Rerun data, e.g.:
//!
//! - Over HTTPS
//! - Over WebSockets
//! - From disk
//!
//! Also handles different file types: rrd, images, text files, 3D models, point clouds…

mod data_loader;
mod data_source;
mod load_file;
mod web_sockets;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

pub use self::data_loader::{
    iter_loaders, register_custom_data_loader, ArchetypeLoader, DataLoader, DataLoaderError,
    DataLoaderSettings, DirectoryLoader, LoadedData, RrdLoader,
};
pub use self::data_source::DataSource;
pub use self::load_file::{extension, load_from_file_contents};
pub use self::web_sockets::connect_to_ws_url;

#[cfg(not(target_arch = "wasm32"))]
pub use self::data_loader::{
    iter_external_loaders, ExternalLoader, EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE,
    EXTERNAL_DATA_LOADER_PREFIX,
};

#[cfg(not(target_arch = "wasm32"))]
pub use self::load_file::load_from_path;

// ---

/// The contents of a file.
///
/// This is what you get when loading a file on Web, or when using drag-n-drop.
//
// TODO(#4554): drag-n-drop streaming support
#[derive(Clone)]
pub struct FileContents {
    pub name: String,
    pub bytes: std::sync::Arc<[u8]>,
}

impl std::fmt::Debug for FileContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileContents")
            .field("name", &self.name)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}

// …given that all feature flags are turned on for the `image` crate.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "dds", "exr", "farbfeld", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "pam",
    "pbm", "pgm", "png", "ppm", "tga", "tif", "tiff", "webp",
];

pub const SUPPORTED_MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj", "stl"];

// TODO(#4532): `.ply` data loader should support 2D point cloud & meshes
pub const SUPPORTED_POINT_CLOUD_EXTENSIONS: &[&str] = &["ply"];

pub const SUPPORTED_RERUN_EXTENSIONS: &[&str] = &["rbl", "rrd"];

// TODO(#4555): Add catch-all builtin `DataLoader` for text files
pub const SUPPORTED_TEXT_EXTENSIONS: &[&str] = &["txt", "md"];

/// All file extension supported by our builtin [`DataLoader`]s.
pub fn supported_extensions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_RERUN_EXTENSIONS
        .iter()
        .chain(SUPPORTED_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_MESH_EXTENSIONS)
        .chain(SUPPORTED_POINT_CLOUD_EXTENSIONS)
        .chain(SUPPORTED_TEXT_EXTENSIONS)
        .copied()
}

/// Is this a supported file extension by any of our builtin [`DataLoader`]s?
pub fn is_supported_file_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS.contains(&extension)
        || SUPPORTED_MESH_EXTENSIONS.contains(&extension)
        || SUPPORTED_POINT_CLOUD_EXTENSIONS.contains(&extension)
        || SUPPORTED_RERUN_EXTENSIONS.contains(&extension)
        || SUPPORTED_TEXT_EXTENSIONS.contains(&extension)
}
