use std::sync::{Arc, RwLock};

use itertools::{Either, Itertools};
use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

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
        let hashed_indices = indices.iter().map(IndexHash::hash).collect_vec();

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
pub enum Batch<T> {
    SequentialBatch(Vec<T>, ArcIndexHashes),
    IndexedBatch(IntMap<IndexHash, T>),
}

impl<T: Clone> Batch<T> {
    #[inline(never)]
    pub fn new_indexed(
        hashed_indices: &[re_log_types::IndexHash],
        data: &[T],
    ) -> Result<Self, BadBatchError> {
        crate::profile_function!(std::any::type_name::<T>());

        if hashed_indices.len() != data.len() {
            return Err(BadBatchError);
        }

        let map = itertools::izip!(hashed_indices, data)
            .map(|(index_hash, value)| (*index_hash, value.clone()))
            .collect();

        Ok(Self::IndexedBatch(map))
    }

    #[inline(never)]
    pub fn new_sequential(data: &[T]) -> Result<Self, BadBatchError> {
        crate::profile_function!(std::any::type_name::<T>());

        let indices = SharedSequentialIndex::hashes_up_to(data.len());

        Ok(Self::SequentialBatch(data.to_vec(), indices))
    }
}

type ArcIndexHashes = Arc<(Vec<IndexHash>, Vec<Index>)>;

/// A singleton collection of hashed indices and the reverse map
///
/// Since all Sequential batches have the same hash-values for their
/// indicies, we only want to store this once. The storage for these
/// hashes needs to exist to satisfy the way the iterator is exposed.
/// This has the added benefit of only needing to compute these hashes
/// once.
///
/// We don't know the maximize size of a batch apriori, so we pick an
/// arbitrary initial size of `1_000_000`. Beyond that we dynamically grow
/// up to the next power of two that will fit the batch.
///
/// We still track these pre-hashed indices per-batch with an Arc to
/// avoid needing to perform unsafe lifetime shenanigans. Resize-operations
/// are guarded by a RW-lock.
pub(crate) struct SharedSequentialIndex {
    hashed_indices: ArcIndexHashes,
    reverse_index_hash_map: IntMap<IndexHash, usize>,
}

const INITIAL_SEQUENTIAL_BATCH_SIZE: usize = 1;

impl SharedSequentialIndex {
    /// Static global accessor
    fn global() -> &'static RwLock<SharedSequentialIndex> {
        static INSTANCE: OnceCell<RwLock<SharedSequentialIndex>> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            let raw_indices = (0..INITIAL_SEQUENTIAL_BATCH_SIZE).collect_vec();

            let indices = raw_indices
                .iter()
                .map(|i| Index::Sequence(*i as u64))
                .collect_vec();

            let hashed_indices = indices.iter().map(IndexHash::hash).collect_vec();

            let reverse_index_hash_map = std::iter::zip(&hashed_indices, raw_indices)
                .map(|(hash, index)| (*hash, index))
                .collect();

            RwLock::new(SharedSequentialIndex {
                hashed_indices: Arc::new((hashed_indices, indices)),
                reverse_index_hash_map,
            })
        })
    }

    /// Increase the hashes up to the required length.
    ///
    /// Holds the write-Lock
    fn grow_hashes_to(len: usize) -> ArcIndexHashes {
        // We could theoretically grab the write-lock only after we have already
        // computed the hashes, but doing so adds other race conditions such as
        // the possibility of multiple threads computing new hashes concurrently.
        let mut global = Self::global().write().unwrap();

        let cur_len = global.hashed_indices.0.len();
        let mut new_len = cur_len;

        while new_len < len {
            new_len *= 2;
        }

        if new_len != cur_len {
            // Start with the current hashes
            let mut new_hashes = (*global.hashed_indices).clone();

            // Extend from the current length to the new length
            // Update the index_hash_map as a side-effect
            new_hashes.extend((cur_len..new_len).map(|i| {
                let seq = Index::Sequence(i as u64);
                let hash = IndexHash::hash(&seq);
                global.reverse_index_hash_map.insert(hash, i);
                (hash, seq)
            }));

            global.hashed_indices = Arc::new(new_hashes);
        }

        global.hashed_indices.clone()
    }

    /// Get all the hashes up to the requested length
    ///
    /// Holds the read-lock
    pub(crate) fn hashes_up_to(len: usize) -> ArcIndexHashes {
        let global = Self::global().read().unwrap();

        if len > global.hashed_indices.0.len() {
            // Drop the read guard so we don't deadlock trying to grow the hashes
            drop(global);
            Self::grow_hashes_to(len)
        } else {
            global.hashed_indices.clone()
        }
    }

    /// Reverses the Hash and returns the original index
    ///
    /// Holds the read-lock
    fn reverse_hash(index: &IndexHash) -> Option<usize> {
        let global = Self::global().read().unwrap();
        Some(*global.reverse_index_hash_map.get(index)?)
    }
}

#[derive(Copy, Clone)]
pub enum BatchIndexLookup<'a> {
    ByIndex(&'a Index),
    ByHash(&'a IndexHash),
}

impl<T> Batch<T> {
    #[inline]
    pub fn get(&self, index: &IndexHash) -> Option<&T> {
        match &self {
            Self::SequentialBatch(vec, _) => vec.get(SharedSequentialIndex::reverse_hash(index)?),
            Self::IndexedBatch(map) => map.get(index),
        }
    }

    #[inline]
    pub fn get_index(&self, index: &Index) -> Option<&T> {
        match &self {
            Self::SequentialBatch(vec, _) => {
                if let Index::Sequence(index) = index {
                    vec.get(*index as usize)
                } else {
                    re_log::error_once!(
                        "Attempted to access Sequential Batch with non-Sequence Index"
                    );
                    None
                }
            }
            Self::IndexedBatch(map) => {
                let index_hash = IndexHash::hash(index);
                map.get(&index_hash)
            }
        }
    }

    #[inline]
    pub fn get_batch(&self, best: BatchIndexLookup<'_>) -> Option<&T> {
        match best {
            BatchIndexLookup::ByIndex(index) => self.get_index(index),
            BatchIndexLookup::ByHash(hash) => self.get(hash),
        }
    }

    #[inline]
    pub fn values(&self) -> impl ExactSizeIterator<Item = &T> {
        match &self {
            Self::SequentialBatch(vec, _) => Either::Left(vec.iter()),
            Self::IndexedBatch(map) => Either::Right(map.values()),
        }
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexHash, BatchIndexLookup<'_>, &T)> {
        match &self {
            Self::SequentialBatch(vec, hashes) => Either::Left(
                itertools::izip!(hashes.0.iter(), hashes.1.iter(), vec)
                    .map(|(h, i, v)| (h, BatchIndexLookup::ByIndex(i), v)),
            ),
            Self::IndexedBatch(map) => {
                Either::Right(map.iter().map(|(h, v)| (h, BatchIndexLookup::ByHash(h), v)))
            }
        }
    }
}
