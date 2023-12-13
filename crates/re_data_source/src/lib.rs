//! Handles different ways of loading Rerun data, e.g.:
//!
//! - Over HTTPS
//! - Over WebSockets
//! - From disk
//!
//! Also handles different file types: rrd, images, text files, 3D models, point clouds…

// TODO: ideally all these loaders would be available from SDKs...

// TODO: we're taking care of one thing today: native filesystem paths.
// -> make it available to SDKs somehow
// -> in the future: any URIs on native
// -> any URIs on the web

// TODO: PR train
// -> stdio streaming
// -> introduction of URI loader + port existing stuff
// -> new URI loaders (txt, md, ply, dir, more?)

// TODO: what happens once we want DataLoader to handle more than filesystem paths?
// One solution is for each loader to support multiple data sources (which is probably best for
// executable loaders).
// Another solution is to have one loader trait per datasource, which kinda sucks.
//
// The first solution would make the dedicated HTTP loading stuff redundant.
//
// At some point we just want to support loading any URIs on both native and web.

// TODO: PR train planning

mod data_loader;
mod data_source;
mod web_sockets;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

pub use data_source::DataSource;
pub use web_sockets::connect_to_ws_url;

pub use self::data_loader::{
    load_from_file_contents, DataLoader, DataLoaderError, LoadedData, BUILTIN_LOADERS,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use self::data_loader::{load_from_file, EXTERNAL_LOADERS};

// ---

/// The contents of as file.
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

pub const SUPPORTED_POINT_CLOUD_EXTENSIONS: &[&str] = &["ply"];

pub const SUPPORTED_RERUN_EXTENSIONS: &[&str] = &["rrd"];

pub const SUPPORTED_TEXT_EXTENSIONS: &[&str] = &["txt", "md"];

pub(crate) fn is_known_file_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS.contains(&extension)
        || SUPPORTED_MESH_EXTENSIONS.contains(&extension)
        || SUPPORTED_POINT_CLOUD_EXTENSIONS.contains(&extension)
        || SUPPORTED_RERUN_EXTENSIONS.contains(&extension)
        || SUPPORTED_TEXT_EXTENSIONS.contains(&extension)
}
