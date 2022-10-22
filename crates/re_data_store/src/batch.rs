use std::sync::Arc;

use itertools::Either;
use lazy_static::lazy_static;
use nohash_hasher::IntMap;

use re_log_types::{Index, IndexHash};

/// The number indices and data were of different lengths.
#[derive(Clone, Copy, Debug)]
pub struct BadBatchError;

pub type ArcBatch<T> = Arc<Batch<T>>;

/// The value of a multi-object field at some time point.
#[derive(Clone)]
pub enum BatchOrSplat<T> {
    /// Splat the same value for every instance of a multi-object.
    Splat(T),

    /// Individual values for all instances of a multi-object.
    Batch(ArcBatch<T>),
}

impl<T: Clone> BatchOrSplat<T> {
    pub fn new_batch(indices: &[re_log_types::Index], data: &[T]) -> Result<Self, BadBatchError> {
        let hashed_indices = indices
            .iter()
            .map(|index| (IndexHash::hash(index), index))
            .collect::<Vec<_>>();

        Ok(Self::Batch(Arc::new(Batch::new_indexed(
            &hashed_indices,
            data,
        )?)))
    }

    pub fn new_sequential_batch(data: &[T]) -> Result<Self, BadBatchError> {
        Ok(Self::Batch(Arc::new(Batch::new_sequential(data)?)))
    }
}

// ----------------------------------------------------------------------------

/// Each [`Index`] in a batch corresponds to an instance of a multi-object.
///
/// Can be shared between different timelines with [`ArcBatch`].
pub struct Batch<T> {
    store: BatchStorage<T>,
}

enum BatchStorage<T> {
    SequentialBatch(Vec<T>),
    IndexedBatch(IntMap<IndexHash, T>),
}

impl<T: Clone> Batch<T> {
    #[inline(never)]
    pub fn new_indexed(
        hashed_indices: &[(re_log_types::IndexHash, &re_log_types::Index)],
        data: &[T],
    ) -> Result<Self, BadBatchError> {
        crate::profile_function!(std::any::type_name::<T>());

        if hashed_indices.len() != data.len() {
            return Err(BadBatchError);
        }

        let map = itertools::izip!(hashed_indices, data)
            .map(|(index_hash, value)| (index_hash.0, value.clone()))
            .collect();

        Ok(Self {
            store: BatchStorage::IndexedBatch(map),
        })
    }

    #[inline(never)]
    pub fn new_sequential(data: &[T]) -> Result<Self, BadBatchError> {
        crate::profile_function!(std::any::type_name::<T>());

        if data.len() > *MAX_SEQUENTIAL_BATCH {
            re_log::warn!("Could not process sequential batch of len {}. Max supported sequential batch size: {}", data.len(), *MAX_SEQUENTIAL_BATCH);
        }

        let vec = data[..std::cmp::min(data.len(), *MAX_SEQUENTIAL_BATCH)].to_vec();

        Ok(Self {
            store: BatchStorage::SequentialBatch(vec),
        })
    }
}

// TODO(jleibs) This is hacky but we simply pre-allocate the index hashings and
// reverse maps arbitrarily up to a million. Ideally we would auto-grow these
// entities to match the largest dense set we need to work with but that gets
// into some complex lifetime questions.
lazy_static! {
    static ref MAX_SEQUENTIAL_BATCH: usize = 1000000;
    static ref HASHED_INDEX: Vec<IndexHash> = (0..*MAX_SEQUENTIAL_BATCH)
        .map(|i| IndexHash::hash(&Index::Sequence(i as u64)))
        .collect();
    static ref REVERSE_INDEX_HASH_MAP: IntMap<IndexHash, usize> = HASHED_INDEX
        .iter()
        .enumerate()
        .map(|(index, hash)| (*hash, index))
        .collect();
}

impl<T> Batch<T> {
    #[inline]
    pub fn get(&self, index: &IndexHash) -> Option<&T> {
        match &self.store {
            BatchStorage::SequentialBatch(vec) => vec.get(*REVERSE_INDEX_HASH_MAP.get(index)?),
            BatchStorage::IndexedBatch(map) => map.get(index),
        }
    }

    #[inline]
    pub fn get_index(&self, index: &Index) -> Option<&T> {
        match &self.store {
            BatchStorage::SequentialBatch(vec) => {
                if let Index::Sequence(index) = index {
                    vec.get(*index as usize)
                } else {
                    None
                }
            }
            BatchStorage::IndexedBatch(map) => {
                let index_hash = IndexHash::hash(index);
                map.get(&index_hash)
            }
        }
    }

    #[inline]
    pub fn values(&self) -> impl ExactSizeIterator<Item = &T> {
        match &self.store {
            BatchStorage::SequentialBatch(vec) => Either::Left(vec.iter()),
            BatchStorage::IndexedBatch(map) => Either::Right(map.values()),
        }
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexHash, &T)> {
        match &self.store {
            BatchStorage::SequentialBatch(vec) => {
                Either::Left(std::iter::zip(&HASHED_INDEX[..vec.len()], vec))
            }
            BatchStorage::IndexedBatch(map) => Either::Right(map.iter()),
        }
    }
}
