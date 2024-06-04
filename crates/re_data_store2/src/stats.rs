use std::sync::Arc;

use re_chunk::Chunk;
use re_types_core::SizeBytes;

use crate::DataStore2;

// ---

#[derive(Debug, Clone, Copy)]
pub struct DataStoreChunkStats2 {
    pub num_chunks: u64,

    pub min_size_bytes: u64,
    pub max_size_bytes: u64,
    pub total_size_bytes: u64,

    pub min_num_rows: u64,
    pub max_num_rows: u64,
    pub total_num_rows: u64,

    pub min_num_components: u64,
    pub max_num_components: u64,

    pub min_num_timelines: u64,
    pub max_num_timelines: u64,
}

impl std::fmt::Display for DataStoreChunkStats2 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            num_chunks,
            min_size_bytes,
            max_size_bytes,
            total_size_bytes,
            min_num_rows,
            max_num_rows,
            total_num_rows,
            min_num_components,
            max_num_components,
            min_num_timelines,
            max_num_timelines,
        } = *self;

        f.write_fmt(format_args!(
            "num_chunks: {}\n",
            re_format::format_uint(num_chunks)
        ))?;
        f.write_fmt(format_args!(
            "min_size_bytes: {}\n",
            re_format::format_bytes(min_size_bytes as _)
        ))?;
        f.write_fmt(format_args!(
            "max_size_bytes: {}\n",
            re_format::format_bytes(max_size_bytes as _)
        ))?;
        f.write_fmt(format_args!(
            "total_size_bytes: {}\n",
            re_format::format_bytes(total_size_bytes as _)
        ))?;
        f.write_fmt(format_args!(
            "min_num_rows: {}\n",
            re_format::format_uint(min_num_rows)
        ))?;
        f.write_fmt(format_args!(
            "max_num_rows: {}\n",
            re_format::format_uint(max_num_rows)
        ))?;
        f.write_fmt(format_args!(
            "total_num_rows: {}\n",
            re_format::format_uint(total_num_rows)
        ))?;
        f.write_fmt(format_args!(
            "min_num_components: {}\n",
            re_format::format_uint(min_num_components)
        ))?;
        f.write_fmt(format_args!(
            "max_num_components: {}\n",
            re_format::format_uint(max_num_components)
        ))?;
        f.write_fmt(format_args!(
            "min_num_timelines: {}\n",
            re_format::format_uint(min_num_timelines)
        ))?;
        f.write_fmt(format_args!(
            "max_num_timelines: {}\n",
            re_format::format_uint(max_num_timelines)
        ))?;

        Ok(())
    }
}

impl Default for DataStoreChunkStats2 {
    fn default() -> Self {
        Self {
            num_chunks: 0,
            min_size_bytes: u64::MAX,
            max_size_bytes: u64::MIN,
            total_size_bytes: 0,
            min_num_rows: u64::MAX,
            max_num_rows: u64::MIN,
            total_num_rows: 0,
            min_num_components: u64::MAX,
            max_num_components: u64::MIN,
            min_num_timelines: u64::MAX,
            max_num_timelines: u64::MIN,
        }
    }
}

impl std::ops::Add for DataStoreChunkStats2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks + rhs.num_chunks,

            min_size_bytes: u64::min(self.min_size_bytes, rhs.min_size_bytes),
            max_size_bytes: u64::max(self.max_size_bytes, rhs.max_size_bytes),
            total_size_bytes: self.total_size_bytes + rhs.total_size_bytes,

            min_num_rows: u64::min(self.min_num_rows, rhs.min_num_rows),
            max_num_rows: u64::max(self.max_num_rows, rhs.max_num_rows),
            total_num_rows: self.total_num_rows + rhs.total_num_rows,

            min_num_components: u64::min(self.min_num_components, rhs.min_num_components),
            max_num_components: u64::max(self.max_num_components, rhs.max_num_components),

            min_num_timelines: u64::min(self.min_num_timelines, rhs.min_num_timelines),
            max_num_timelines: u64::max(self.max_num_timelines, rhs.max_num_timelines),
        }
    }
}

impl std::ops::AddAssign for DataStoreChunkStats2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub for DataStoreChunkStats2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            num_chunks: self.num_chunks - rhs.num_chunks,

            min_size_bytes: u64::min(self.min_size_bytes, rhs.min_size_bytes),
            max_size_bytes: u64::max(self.max_size_bytes, rhs.max_size_bytes),
            total_size_bytes: self.total_size_bytes - rhs.total_size_bytes,

            min_num_rows: u64::min(self.min_num_rows, rhs.min_num_rows),
            max_num_rows: u64::max(self.max_num_rows, rhs.max_num_rows),
            total_num_rows: self.total_num_rows - rhs.total_num_rows,

            min_num_components: u64::min(self.min_num_components, rhs.min_num_components),
            max_num_components: u64::max(self.max_num_components, rhs.max_num_components),

            min_num_timelines: u64::min(self.min_num_timelines, rhs.min_num_timelines),
            max_num_timelines: u64::max(self.max_num_timelines, rhs.max_num_timelines),
        }
    }
}

impl std::ops::SubAssign for DataStoreChunkStats2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl DataStoreChunkStats2 {
    #[inline]
    pub fn from_chunk(chunk: &Arc<Chunk>) -> Self {
        let size_bytes = <Chunk as SizeBytes>::total_size_bytes(&**chunk);
        let num_rows = chunk.num_rows() as u64;
        let num_components = chunk.num_components() as u64;
        let num_timelines = chunk.num_timelines() as u64;

        Self {
            num_chunks: 1,

            min_size_bytes: size_bytes,
            max_size_bytes: size_bytes,
            total_size_bytes: size_bytes,

            min_num_rows: num_rows,
            max_num_rows: num_rows,
            total_num_rows: num_rows,

            min_num_components: num_components,
            max_num_components: num_components,

            min_num_timelines: num_timelines,
            max_num_timelines: num_timelines,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct DataStoreStats2 {
    pub static_chunks: DataStoreChunkStats2,
    pub temporal_chunks: DataStoreChunkStats2,

    pub total: DataStoreChunkStats2,
}

impl std::ops::Add for DataStoreStats2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
            total: _,
        } = self;

        let static_chunks = static_chunks + rhs.static_chunks;
        let temporal_chunks = temporal_chunks + rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
            total: static_chunks + temporal_chunks,
        }
    }
}

impl std::ops::Sub for DataStoreStats2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        let Self {
            static_chunks,
            temporal_chunks,
            total: _,
        } = self;

        let static_chunks = static_chunks - rhs.static_chunks;
        let temporal_chunks = temporal_chunks - rhs.temporal_chunks;

        Self {
            static_chunks,
            temporal_chunks,
            total: static_chunks + temporal_chunks,
        }
    }
}

impl DataStore2 {
    #[inline]
    pub fn stats(&self) -> DataStoreStats2 {
        DataStoreStats2 {
            static_chunks: self.static_chunks_stats,
            temporal_chunks: self.temporal_chunks_stats,
            total: self.static_chunks_stats + self.temporal_chunks_stats,
        }
    }
}
