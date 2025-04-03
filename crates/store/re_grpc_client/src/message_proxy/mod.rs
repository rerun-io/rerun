pub mod read;
pub use read::stream;

#[cfg(not(target_arch = "wasm32"))]
pub mod write;

#[cfg(not(target_arch = "wasm32"))]
pub use write::{Client, TableClient};
