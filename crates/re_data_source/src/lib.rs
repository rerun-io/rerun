//! Handles different ways of loading Rerun data, e.g.:
//!
//! - Over HTTPS
//! - Over WebSockets
//! - From disk
//!
//! Also handles different file types:
//!
//! - .rrd
//! - images
//! - meshes

mod data_loader;
mod data_source;
mod load_file;
mod web_sockets;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

pub use self::data_loader::{
    iter_loaders, ArchetypeLoader, DataLoader, DataLoaderError, LoadedData, RrdLoader,
};
pub use self::data_source::DataSource;
pub use self::load_file::{extension, load_from_file_contents};
pub use self::web_sockets::connect_to_ws_url;

#[cfg(not(target_arch = "wasm32"))]
pub use self::load_file::load_from_path;

// ---

/// The contents of a file.
///
/// This is what you get when loading a file on Web, or when using drag-n-drop.
#[derive(Clone, Debug)]
pub struct FileContents {
    pub name: String,
    pub bytes: std::sync::Arc<[u8]>,
}

// …given that all feature flags are turned on for the `image` crate.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "dds", "exr", "farbfeld", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "pam",
    "pbm", "pgm", "png", "ppm", "tga", "tif", "tiff", "webp",
];

pub const SUPPORTED_MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj"];

pub const SUPPORTED_RERUN_EXTENSIONS: &[&str] = &["rrd"];

/// All file extension supported by our builtin [`DataLoader`]s.
pub fn supported_extensions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_RERUN_EXTENSIONS
        .iter()
        .chain(SUPPORTED_IMAGE_EXTENSIONS)
        .chain(SUPPORTED_MESH_EXTENSIONS)
        .copied()
}

/// Is this a supported file extension by any of our builtin [`DataLoader`]s?
pub fn is_supported_file_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS.contains(&extension)
        || SUPPORTED_MESH_EXTENSIONS.contains(&extension)
        || SUPPORTED_RERUN_EXTENSIONS.contains(&extension)
}
