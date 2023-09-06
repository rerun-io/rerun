use smallvec::SmallVec;

use crate::datatypes::{TensorData, TensorDimension};

#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<TensorData>> {
    #[error("Could not convert source to TensorData")]
    TensorDataConversion(T::Error),

    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(Vec<TensorDimension>),
}

// Returns the indices of an appropriate set of non-empty dimensions
pub fn find_non_empty_dim_indices(shape: &Vec<TensorDimension>) -> SmallVec<[usize; 4]> {
    if shape.len() < 2 {
        return SmallVec::<_>::new();
    }

    let mut iter_non_empty =
        shape
            .iter()
            .enumerate()
            .filter_map(|(ind, dim)| if dim.size != 1 { Some(ind) } else { None });

    // 0 must be valid since shape isn't empty or we would have returned an Err above
    let mut first_non_empty = iter_non_empty.next().unwrap_or(0);
    let mut last_non_empty = iter_non_empty.last().unwrap_or(first_non_empty);

    // Note, these are inclusive ranges.

    // First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    // Grow to a min-size of 2.
    // (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while last_non_empty - first_non_empty < 1 && last_non_empty < (shape.len() - 1) {
        last_non_empty += 1;
    }

    // Next, consider empty outer dimensions if we still need them.
    // Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    // Otherwise, only grow up to 2.
    // (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    let target = match shape[last_non_empty].size {
        3 | 4 => 2,
        _ => 1,
    };

    while last_non_empty - first_non_empty < target && first_non_empty > 0 {
        first_non_empty -= 1;
    }

    (first_non_empty..=last_non_empty).collect()
}
