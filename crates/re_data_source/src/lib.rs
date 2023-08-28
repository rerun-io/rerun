mod data_source;

mod load_file_contents;
mod web_sockets;

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
