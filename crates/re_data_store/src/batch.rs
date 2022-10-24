use std::sync::Arc;

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

        Ok(Self::Batch(Arc::new(Batch::new(&hashed_indices, data)?)))
    }
}

// ----------------------------------------------------------------------------

/// Each [`Index`] in a batch corresponds to an instance of a multi-object.
///
/// Can be shared between different timelines with [`ArcBatch`].
pub struct Batch<T> {
    map: IntMap<IndexHash, T>,
}

impl<T: Clone> Batch<T> {
    #[inline(never)]
    pub fn new(
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

        Ok(Self { map })
    }
}

impl<T> Batch<T> {
    #[inline]
    pub fn get(&self, index: &IndexHash) -> Option<&T> {
        self.map.get(index)
    }

    #[inline]
    pub fn get_index(&self, index: &Index) -> Option<&T> {
        let index_hash = IndexHash::hash(index);
        self.map.get(&index_hash)
    }

    #[inline]
    pub fn values(&self) -> impl ExactSizeIterator<Item = &T> {
        self.map.values()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexHash, &T)> {
        self.map.iter()
    }
}
