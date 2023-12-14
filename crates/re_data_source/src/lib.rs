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

mod data_source;

mod load_file;
mod load_file_contents;
mod web_sockets;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

#[cfg(not(target_arch = "wasm32"))]
mod load_file_path;

pub use data_source::DataSource;
pub use web_sockets::connect_to_ws_url;

/// The contents of as file
#[derive(Clone)]
pub struct FileContents {
    pub name: String,

    pub bytes: std::sync::Arc<[u8]>,
}

pub const SUPPORTED_MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj"];

// …given that all feature flags are turned on for the `image` crate.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "dds", "exr", "farbfeld", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "pam",
    "pbm", "pgm", "png", "ppm", "tga", "tif", "tiff", "webp",
];
