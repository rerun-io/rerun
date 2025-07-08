//! Browser UI for [`re_chunk_store::ChunkStore`].

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod chunk_list_mode;
mod chunk_store_ui;
mod chunk_ui;
mod sort;

pub use chunk_store_ui::DatastoreUi;
