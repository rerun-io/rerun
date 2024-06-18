use crate::{Chunk, ChunkId, ChunkResult};

// ---

// TODO: maybe this just doesn't make sense really
#[cfg(TODO)]
impl Chunk {
    // TODO: what's the name of that???
    pub fn splat_merge(lhs: &Self, rhs: &Self) -> ChunkResult<Self> {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids,
            timelines,
            components,
        } = lhs;

        if entity_path != rhs.entity_path() {
            panic!("TODO");
        }

        // TODO: hmm, how does this work index-wise... unless we do a "splat merge" thing huehuehuhe
        //
        // we have to do some kind of range_zip i guess

        Self::new(
            ChunkId::new(),
            entity_path,
            is_sorted,
            row_ids,
            timelines,
            components,
        )
    }
}
