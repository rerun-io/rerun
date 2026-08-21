//! A transparent wrapper around a nullable field has no validity bitmap of its own: the array's
//! nulls *are* that field's `None`s.
//!
//! The non-nullable `ToArrow` / `FromArrow` path must therefore still build and read that bitmap,
//! rather than assuming an absence of nulls the way it can for every other type.

use re_sdk_types::testing::encodings::TransparentOptionalFloat;
use re_sdk_types::{FromArrow as _, ToArrow as _};

#[test]
fn none_survives_a_roundtrip() {
    let input = vec![
        TransparentOptionalFloat(Some(1.0)),
        TransparentOptionalFloat(None),
        TransparentOptionalFloat(Some(3.0)),
    ];

    let array = TransparentOptionalFloat::to_arrow(input.clone()).unwrap();
    assert_eq!(
        arrow::array::Array::null_count(array.as_ref()),
        1,
        "the `None` must be encoded as an Arrow null"
    );

    let output = TransparentOptionalFloat::from_arrow(array.as_ref()).unwrap();
    assert_eq!(output, input);
}

/// All-`Some` data needs no validity bitmap at all.
#[test]
fn no_nulls_means_no_bitmap() {
    let input = vec![
        TransparentOptionalFloat(Some(1.0)),
        TransparentOptionalFloat(Some(2.0)),
    ];

    let array = TransparentOptionalFloat::to_arrow(input.clone()).unwrap();
    assert_eq!(arrow::array::Array::null_count(array.as_ref()), 0);
    assert!(array.nulls().is_none());

    let output = TransparentOptionalFloat::from_arrow(array.as_ref()).unwrap();
    assert_eq!(output, input);
}
