//! Browser UI for [`re_chunk_store::ChunkStore`].

mod arrow_ui;
mod chunk_list_mode;
mod datastore_ui;

pub use datastore_ui::DatastoreUi;

// --

//TODO(ab): move that to a more generally accessible/useful place?
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    pub(crate) fn toggle(&mut self) {
        match self {
            Self::Ascending => *self = Self::Descending,
            Self::Descending => *self = Self::Ascending,
        }
    }
}

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascending => " ▼".fmt(f),
            Self::Descending => " ▲".fmt(f),
        }
    }
}
