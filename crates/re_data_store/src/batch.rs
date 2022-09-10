use std::sync::Arc;

use nohash_hasher::IntMap;

use re_log_types::{Index, IndexHash};

pub type ArcBatch<T> = Arc<Batch<T>>;

#[derive(Clone)]
pub enum BatchOrSplat<T> {
    /// Splat the same value for everything
    Splat(T),
    Batch(ArcBatch<T>),
}

impl<T: Clone> BatchOrSplat<T> {
    pub fn new_batch(indices: &[re_log_types::Index], data: &[T]) -> Self {
        Self::Batch(Arc::new(Batch::new(indices, data)))
    }
}

// ----------------------------------------------------------------------------

/// Can be shared between different timelines with [`ArcBatch`].
///
/// Each [`Index`] in a batch corresponds to an instance of a multi-object.
pub struct Batch<T> {
    map: IntMap<IndexHash, T>,
    hashed_indices: Vec<(IndexHash, Index)>,
}

impl<T: Clone> Batch<T> {
    #[inline(never)]
    pub fn new(indices: &[re_log_types::Index], data: &[T]) -> Self {
        crate::profile_function!(std::any::type_name::<T>());

        assert_eq!(indices.len(), data.len()); // TODO(emilk): return Result instead
        let mut hashed_indices = Vec::with_capacity(indices.len());
        let map = itertools::izip!(indices, data)
            .map(|(index, value)| {
                let index_hash = IndexHash::hash(index);
                hashed_indices.push((index_hash, index.clone()));
                (index_hash, value.clone())
            })
            .collect();
        Self {
            map,
            hashed_indices,
        }
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
