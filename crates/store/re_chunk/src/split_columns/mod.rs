//! Splitting chunks column-wise to optimize for certain queries.
//!
//! Two rules, applied in order:
//! * a component that wants a chunk of its own gets one (`own_chunk`);
//! * what is left is split along thick/thin size tiers (`thick_thin`).
//!
//! Every piece keeps all the rows. Contrast [`Chunk::split_rows`], which keeps all the columns.
//!
//! This crate knows nothing about any particular component, so the caller says which components
//! want a chunk of their own, via [`SplitColumnsOptions`].

mod own_chunk;
mod thick_thin;

pub use self::own_chunk::may_merge;

use re_types_core::reflection::ComponentTypeSet;

use crate::Chunk;

/// How [`Chunk::split_columns`] should break a chunk up.
#[derive(Clone, Debug, Default)]
pub struct SplitColumnsOptions {
    /// The component types that always get a chunk of their own.
    ///
    /// `re_sdk_types::reflection::own_chunk_components` has the built-in ones.
    pub own_chunks: ComponentTypeSet,

    /// If set, chunks are split so no two archetype groups sharing a chunk differ in byte size
    /// by more than this factor. `None` disables the thick/thin split.
    pub split_size_ratio: Option<f64>,
}

impl Chunk {
    /// Split this chunk by column, into the pieces it is better off stored as, or `None` to
    /// leave it be.
    ///
    /// This is a heuristic, and purely an optimization: it changes how the same data is laid out
    /// across chunks so that a reader fetches less of what it did not ask for. Nothing depends on
    /// it for correctness, and every layout is a valid one. Contrast
    /// [`Chunk::split_rows`], which splits by row because a chunk grew past a hard
    /// limit.
    ///
    /// A component in [`SplitColumnsOptions::own_chunks`] always gets a chunk of its own, and, if
    /// [`SplitColumnsOptions::split_size_ratio`] is set, each remaining piece is split into one
    /// chunk per size tier.
    ///
    /// All pieces keep all row ids and time columns of the original chunk.
    /// The row ids are needed for joining the pieces back together, and the time columns are
    /// needed for fast indexing in the pieces without joining with other chunks.
    pub fn split_columns(&self, options: &SplitColumnsOptions) -> Option<Vec<Self>> {
        let pieces = own_chunk::split(self, options);

        let Some(ratio) = options.split_size_ratio else {
            return pieces;
        };

        match pieces {
            Some(pieces) => Some(
                pieces
                    .into_iter()
                    .flat_map(|piece| {
                        thick_thin::split(&piece, ratio).unwrap_or_else(|| vec![piece])
                    })
                    .collect(),
            ),
            None => thick_thin::split(self, ratio),
        }
    }
}
