use std::collections::HashMap;

use re_types::{
    archetypes::Tensor,
    datatypes::{TensorBuffer, TensorData},
    tensor_data::TensorCastError,
    Archetype as _, AsComponents as _,
};

use crate::util;

#[test]
fn tensor_roundtrip() {
    let all_expected = [Tensor {
        data: TensorData::new(vec![2, 3], TensorBuffer::U8(vec![1, 2, 3, 4, 5, 6].into())).into(),
        value_range: None,
    }];

    let all_arch_serialized = [Tensor::try_from(ndarray::array![[1u8, 2, 3], [4, 5, 6]])
        .unwrap()
        .to_arrow2()
        .unwrap()];

    let expected_extensions: HashMap<_, _> = [("data", vec!["rerun.components.TensorData"])].into();

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name);

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = Tensor::from_arrow2(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

#[test]
fn convert_tensor_to_ndarray_u8() {
    let t = TensorData::new(vec![3, 4, 5], TensorBuffer::U8(vec![0; 60].into()));

    let n = ndarray::ArrayViewD::<u8>::try_from(&t).unwrap();

    assert_eq!(n.shape(), &[3, 4, 5]);
}

#[test]
fn convert_tensor_to_ndarray_u16() {
    let t = TensorData::new(vec![3, 4, 5], TensorBuffer::U16(vec![0_u16; 60].into()));

    let n = ndarray::ArrayViewD::<u16>::try_from(&t).unwrap();

    assert_eq!(n.shape(), &[3, 4, 5]);
}

#[test]
fn convert_tensor_to_ndarray_f32() {
    let t = TensorData::new(vec![3, 4, 5], TensorBuffer::F32(vec![0_f32; 60].into()));

    let n = ndarray::ArrayViewD::<f32>::try_from(&t).unwrap();

    assert_eq!(n.shape(), &[3, 4, 5]);
}

#[test]
fn convert_ndarray_f64_to_tensor() {
    let n = ndarray::array![[1., 2., 3.], [4., 5., 6.]];
    let t = TensorData::try_from(n).unwrap();

    assert_eq!(t.shape(), &[2, 3]);
}

#[test]
fn convert_ndarray_slice_to_tensor() {
    let n = ndarray::array![[1., 2., 3.], [4., 5., 6.]];
    let n = &n.slice(ndarray::s![.., 1]);
    let t = TensorData::try_from(*n).unwrap();

    assert_eq!(t.shape(), &[2]);
}

#[test]
fn convert_ndarray_to_tensor_both_layouts() {
    #[rustfmt::skip]
    let row_major_vec = vec![
        1, 2, 3,
        4, 5, 6,
        7, 8, 9
    ];
    #[rustfmt::skip]
    let col_major_vec = vec![
        1, 4, 7,
        2, 5, 8,
        3, 6, 9
    ];

    let shape = ndarray::Ix2(3, 3);

    let row_major = ndarray::Array::from_vec(row_major_vec)
        .into_shape_with_order((shape, ndarray::Order::RowMajor))
        .unwrap();

    let col_major = ndarray::Array::from_vec(col_major_vec)
        .into_shape_with_order((shape, ndarray::Order::ColumnMajor))
        .unwrap();

    assert!(row_major.is_standard_layout());
    assert!(!col_major.is_standard_layout());

    // make sure that the offset is in fact zero, in case ndarray behavior changes
    let rm = row_major.clone();
    let cm = col_major.clone();
    let (_, rm_offset) = rm.into_raw_vec_and_offset();
    let (_, cm_offset) = cm.into_raw_vec_and_offset();
    assert_eq!(rm_offset.unwrap(), 0);
    assert_eq!(cm_offset.unwrap(), 0);

    let tensor_row_major = TensorData::try_from(row_major).unwrap();
    let tensor_col_major = TensorData::try_from(col_major).unwrap();

    assert_eq!(tensor_row_major, tensor_col_major);
}

#[test]
fn convert_ndarray_to_tensor_both_layouts_nonzero_offset() {
    #[rustfmt::skip]
    let row_major_vec = vec![
        1, 2, 3,
        4, 5, 6,
        7, 8, 9
    ];
    #[rustfmt::skip]
    let col_major_vec = vec![
        1, 4, 7,
        2, 5, 8,
        3, 6, 9
    ];

    let shape = ndarray::Ix2(3, 3);

    let row_major = ndarray::Array::from_vec(row_major_vec)
        .into_shape_with_order((shape, ndarray::Order::RowMajor))
        .unwrap();
    assert!(row_major.is_standard_layout());
    let row_major_nonzero_offset = row_major.slice_move(ndarray::s![1.., ..]);

    let col_major = ndarray::Array::from_vec(col_major_vec)
        .into_shape_with_order((shape, ndarray::Order::ColumnMajor))
        .unwrap();
    assert!(!col_major.is_standard_layout());
    let col_major_nonzero_offset = col_major.slice_move(ndarray::s![1.., ..]);

    assert!(row_major_nonzero_offset.is_standard_layout());
    assert!(!col_major_nonzero_offset.is_standard_layout());

    // make sure that the offset is in fact non-zero, in case ndarray behavior changes
    let rmno = row_major_nonzero_offset.clone();
    let cmno = col_major_nonzero_offset.clone();
    let (_, rm_offset) = rmno.into_raw_vec_and_offset();
    let (_, cm_offset) = cmno.into_raw_vec_and_offset();
    assert!(rm_offset.unwrap() > 0);
    assert!(cm_offset.unwrap() > 0);

    let tensor_row_major_nonzero_offset = TensorData::try_from(row_major_nonzero_offset).unwrap();
    let tensor_col_major_nonzero_offset = TensorData::try_from(col_major_nonzero_offset).unwrap();

    assert_eq!(
        tensor_row_major_nonzero_offset,
        tensor_col_major_nonzero_offset
    );
}

#[test]
fn check_slices() {
    let t = TensorData::new(
        vec![3, 4, 5],
        TensorBuffer::U16((0_u16..60).collect::<Vec<u16>>().into()),
    );

    let n = ndarray::ArrayViewD::<u16>::try_from(&t).unwrap();

    // First element should be 0
    assert_eq!(n[[0, 0, 0]], 0);
    // Last element should be 59
    assert_eq!(n[[2, 3, 4]], 59);

    // Try all the indices:
    for z in 0..3 {
        for y in 0..4 {
            for x in 0..5 {
                assert_eq!(n[[z, y, x]] as usize, z * 4 * 5 + y * 5 + x);
            }
        }
    }

    // Slice the tensor
    let sl: ndarray::ArrayView2<'_, u16> = n.slice(ndarray::s![.., 1, ..]);

    // New slice is missing the middle dimension
    assert_eq!(sl.shape(), &[3, 5]);

    // Equivalent to (0,1,0) = 0*20 + 1*5 + 0 = 5
    assert_eq!(sl[[0, 0]], 5);
    // Equivalent to (1,1,3) = 1*20 + 1*5 + 3 = 28
    assert_eq!(sl[[1, 3]], 28);
}

#[test]
fn check_tensor_shape_error() {
    let t = TensorData::new(vec![3, 4, 5], TensorBuffer::U8(vec![0; 59].into()));

    let n = ndarray::ArrayViewD::<u8>::try_from(&t);

    assert_eq!(
        n,
        Err(TensorCastError::BadTensorShape {
            source: ndarray::ShapeError::from_kind(ndarray::ErrorKind::OutOfBounds)
        })
    );
}

#[test]
fn check_tensor_type_error() {
    let t = TensorData::new(vec![3, 4, 5], TensorBuffer::U16(vec![0; 60].into()));

    let n = ndarray::ArrayViewD::<u8>::try_from(&t);

    assert_eq!(n, Err(TensorCastError::TypeMismatch));
}
