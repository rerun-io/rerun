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
        Ok(Self::Batch(Arc::new(Batch::new(indices, data)?)))
    }
}

// ----------------------------------------------------------------------------

/// Each [`Index`] in a batch corresponds to an instance of a multi-object.
///
/// Can be shared between different timelines with [`ArcBatch`].
pub struct Batch<T> {
    map: IntMap<IndexHash, T>,
    hashed_indices: Vec<(IndexHash, Index)>,
}

impl<T: Clone> Batch<T> {
    #[inline(never)]
    pub fn new(indices: &[re_log_types::Index], data: &[T]) -> Result<Self, BadBatchError> {
        crate::profile_function!(std::any::type_name::<T>());

        if indices.len() != data.len() {
            return Err(BadBatchError);
        }

        let mut hashed_indices = Vec::with_capacity(indices.len());
        let map = itertools::izip!(indices, data)
            .map(|(index, value)| {
                let index_hash = IndexHash::hash(index);
                hashed_indices.push((index_hash, index.clone()));
                (index_hash, value.clone())
            })
            .collect();
        Ok(Self {
            map,
            hashed_indices,
        })
    }
}

impl<T> Batch<T> {
    #[inline]
    pub fn get(&self, index: &IndexHash) -> Option<&T> {
        self.map.get(index)
    }

    #[inline]
    pub fn indices(&self) -> std::slice::Iter<'_, (IndexHash, Index)> {
        self.hashed_indices.iter()
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
