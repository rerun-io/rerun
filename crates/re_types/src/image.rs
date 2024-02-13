use smallvec::{smallvec, SmallVec};

use crate::datatypes::{TensorData, TensorDimension};

#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<TensorData>>
where
    T::Error: std::error::Error,
{
    #[error("Could not convert source to TensorData: {0}")]
    TensorDataConversion(T::Error),

    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(Vec<TensorDimension>),
}

/// Returns the indices of an appropriate set of dimensions.
///
/// Ignores leading and trailing 1-sized dimensions.
///
/// For instance: `[1, 640, 480, 3, 1]` would return `[1, 2, 3]`,
/// the indices of the `[640, 480, 3]` dimensions.
pub fn find_non_empty_dim_indices(shape: &[TensorDimension]) -> SmallVec<[usize; 4]> {
    match shape.len() {
        0 => return smallvec![],
        1 => return smallvec![0],
        2 => return smallvec![0, 1],
        _ => {}
    }

    // Find a range of non-unit dimensions.
    // [1, 1, 1, 640, 480, 3, 1, 1, 1]
    //           ^---------^   goal range

    let mut non_unit_indices =
        shape
            .iter()
            .enumerate()
            .filter_map(|(ind, dim)| if dim.size != 1 { Some(ind) } else { None });

    // 0 is always a valid index.
    let mut min = non_unit_indices.next().unwrap_or(0);
    let mut max = non_unit_indices.last().unwrap_or(min);

    // Note, these are inclusive ranges.

    // First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    // Grow to a min-size of 2.
    // (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while max == min && max + 1 < shape.len() {
        max += 1;
    }

    // Next, consider empty outer dimensions if we still need them.
    // Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    // Otherwise, only grow up to 2.
    // (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    let target_len = match shape[max].size {
        3 | 4 => 3,
        _ => 2,
    };

    while max - min + 1 < target_len && 0 < min {
        min -= 1;
    }

    (min..=max).collect()
}

#[test]
fn test_find_non_empty_dim_indices() {
    fn expect(shape: &[u64], expected: &[usize]) {
        let dim: Vec<_> = shape
            .iter()
            .map(|s| TensorDimension {
                size: *s,
                name: None,
            })
            .collect();
        let got = find_non_empty_dim_indices(&dim);
        assert!(
            got.as_slice() == expected,
            "Input: {shape:?}, got {got:?}, expected {expected:?}"
        );
    }

    expect(&[], &[]);
    expect(&[0], &[0]);
    expect(&[1], &[0]);
    expect(&[100], &[0]);

    expect(&[640, 480], &[0, 1]);
    expect(&[640, 480, 1], &[0, 1]);
    expect(&[640, 480, 1, 1], &[0, 1]);
    expect(&[640, 480, 3], &[0, 1, 2]);
    expect(&[1, 640, 480], &[1, 2]);
    expect(&[1, 640, 480, 3, 1], &[1, 2, 3]);
    expect(&[1, 3, 640, 480, 1], &[1, 2, 3]);
    expect(&[1, 1, 640, 480], &[2, 3]);
    expect(&[1, 1, 640, 480, 1, 1], &[2, 3]);

    expect(&[1, 1, 3], &[0, 1, 2]);
    expect(&[1, 1, 3, 1], &[2, 3]);
}
