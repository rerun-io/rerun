use re_chunk::Chunk;

use crate::{Lens, Lenses, OutputMode, PartialChunk};

/// Extension methods for applying lenses to a [`Chunk`].
pub trait ChunkExt {
    /// Apply one or more lenses to this chunk, returning transformed chunks.
    ///
    /// Each lens matches by input component. Columns not consumed by any
    /// matching lens are forwarded unchanged as a separate chunk
    /// ([`OutputMode::ForwardUnmatched`]).
    ///
    /// If no lens matches the chunk (including when an empty slice is passed),
    /// the original chunk is returned unchanged.
    fn apply_lenses(&self, lenses: &[Lens]) -> Result<Vec<Chunk>, PartialChunk>;
}

impl ChunkExt for Chunk {
    fn apply_lenses(&self, lenses: &[Lens]) -> Result<Vec<Chunk>, PartialChunk> {
        let mut collection = Lenses::new(OutputMode::ForwardUnmatched);
        for lens in lenses {
            collection = collection.add_lens(lens.clone());
        }

        collection.apply(self).collect::<Result<Vec<_>, _>>()
    }
}
