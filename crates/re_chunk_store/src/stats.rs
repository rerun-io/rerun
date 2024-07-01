use std::sync::Arc;

use re_chunk::Chunk;
use re_types_core::SizeBytes;

use crate::ChunkStore;

// ---

#[derive(Default, Debug, Clone, Copy)]
pub struct ChunkStoreStats {
    pub static_chunks: ChunkStoreChunkStats,
    pub temporal_chunks: ChunkStoreChunkStats,
}

impl ChunkStoreStats {
    #[inline]
    pub fn total(&self) -> ChunkStoreChunkStats {
        let Self {
            static_chunks,
            temporal_chunks,
        } = *self;
        static_chunks + temporal_chunks
    }
}

impl std::ops::Add for ChunkStoreStats {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
        } = self;

        let static_chunks = static_chunks + rhs.static_chunks;
        let temporal_chunks = temporal_chunks + rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
        }
    }
}

impl std::ops::Sub for ChunkStoreStats {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
        } = self;

        let static_chunks = static_chunks - rhs.static_chunks;
        let temporal_chunks = temporal_chunks - rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
        }
    }
}

impl ChunkStore {
    #[inline]
    pub fn stats(&self) -> ChunkStoreStats {
        ChunkStoreStats {
            static_chunks: self.static_chunks_stats,
            temporal_chunks: self.temporal_chunks_stats,
        }
    }
}

// ---

#[derive(Default, Debug, Clone, Copy)]
pub struct ChunkStoreChunkStats {
    pub num_chunks: u64,
    pub total_size_bytes: u64,
    pub total_num_rows: u64,
}

impl std::fmt::Display for ChunkStoreChunkStats {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            num_chunks,
            total_size_bytes,
            total_num_rows,
        } = *self;

        f.write_fmt(format_args!(
            "num_chunks: {}\n",
            re_format::format_uint(num_chunks)
        ))?;
        f.write_fmt(format_args!(
            "total_size_bytes: {}\n",
            re_format::format_bytes(total_size_bytes as _)
        ))?;
        f.write_fmt(format_args!(
            "total_num_rows: {}\n",
            re_format::format_uint(total_num_rows)
        ))?;

        Ok(())
    }
}

impl std::ops::Add for ChunkStoreChunkStats {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks + rhs.num_chunks,
            total_size_bytes: self.total_size_bytes + rhs.total_size_bytes,
            total_num_rows: self.total_num_rows + rhs.total_num_rows,
        }
    }
}

impl std::ops::AddAssign for ChunkStoreChunkStats {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub for ChunkStoreChunkStats {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks - rhs.num_chunks,
            total_size_bytes: self.total_size_bytes - rhs.total_size_bytes,
            total_num_rows: self.total_num_rows - rhs.total_num_rows,
        }
    }
}

impl std::ops::SubAssign for ChunkStoreChunkStats {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl ChunkStoreChunkStats {
    #[inline]
    pub fn from_chunk(chunk: &Arc<Chunk>) -> Self {
        // NOTE: Do _NOT_ use `chunk.total_size_bytes` as it is sitting behind an Arc
        // and would count as amortized (i.e. 0 bytes).
        let size_bytes = <Chunk as SizeBytes>::total_size_bytes(&**chunk);
        let num_rows = chunk.num_rows() as u64;

        Self {
            num_chunks: 1,
            total_size_bytes: size_bytes,
            total_num_rows: num_rows,
        }
    }
}
