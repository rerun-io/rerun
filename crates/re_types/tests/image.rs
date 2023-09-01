use std::collections::HashMap;

use re_types::{
    archetypes::Image,
    datatypes::{TensorBuffer, TensorData, TensorDimension, TensorId},
    Archetype as _,
};

mod util;

fn some_id(x: u8) -> TensorId {
    TensorId {
        uuid: [x, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    }
}
#[test]
fn image_roundtrip() {
    let all_expected = [Image {
        data: TensorData {
            id: some_id(0),
            shape: vec![
                TensorDimension {
                    size: 2,
                    name: Some("height".into()),
                },
                TensorDimension {
                    size: 3,
                    name: Some("width".into()),
                },
            ],
            buffer: TensorBuffer::U8(vec![1, 2, 3, 4, 5, 6].into()),
        }
        .into(),
    }];

    let all_arch_serialized = [Image::try_from(ndarray::array![[1u8, 2, 3], [4, 5, 6]])
        .unwrap()
        .with_id(some_id(0))
        .to_arrow()];

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

        let deserialized = Image::try_from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

macro_rules! check_image_array {
    ($img:ty, $typ:ty, $arr:expr, $color_dim:expr) => {{
        let arr = $arr;

        let arrow = <$img>::try_from(arr.clone()).unwrap().to_arrow();

        let img = <$img>::try_from_arrow(arrow).unwrap();

        let color_dim = img
            .data
            .0
            .shape
            .iter()
            .enumerate()
            .find(|(_, dim)| dim.name.as_ref().map(|n| n.as_str()) == Some("color"))
            .map(|(ind, _)| ind as i32)
            .unwrap_or(-1);

        assert_eq!(color_dim, $color_dim);

        let view1 = arr.view().into_dyn();
        let view2 = ndarray::ArrayViewD::<$typ>::try_from(&img).unwrap();

        assert_eq!(view1, view2);
    }};
}

#[test]
fn image_base_ext() {
    // 1x1 -> mono
    check_image_array!(Image, u8, ndarray::array![[4]], -1);
    // 2x3 -> mono
    check_image_array!(Image, u16, ndarray::array![[1, 2, 3], [4, 5, 6]], -1);
    // 1x1x1 -> mono
    check_image_array!(Image, u32, ndarray::array![[[1]]], -1);
    // 1x3x1 -> mono
    check_image_array!(Image, u64, ndarray::array![[[1], [2], [3]]], -1);
    // 1x1x3 -> rgb
    check_image_array!(Image, f32, ndarray::array![[[1.0, 2.0, 3.0]]], 2);
    // 1x1x5 -> mono
    check_image_array!(Image, f64, ndarray::array![[[1.0, 2.0, 3.0, 4.0, 5.0]]], -1);
    // 1x2x3 -> rgb
    check_image_array!(Image, u8, ndarray::array![[[1, 2, 3], [4, 5, 6]]], 2);
    // 1x2x4 -> rgba
    check_image_array!(Image, u8, ndarray::array![[[1, 2, 3, 4], [5, 6, 7, 8]]], 2);
    // 1x1x3x1 -> mono
    check_image_array!(
        Image,
        u8,
        ndarray::Array::from_shape_vec((1, 1, 3, 1), vec![1, 2, 3]).unwrap(),
        -1
    );
    // 1x1x1x3 -> rgb
    check_image_array!(
        Image,
        u8,
        ndarray::Array::from_shape_vec((1, 1, 1, 3), vec![1, 2, 3]).unwrap(),
        3
    );
    // 1x1x1x5 -> mono
    check_image_array!(
        Image,
        u8,
        ndarray::Array::from_shape_vec((1, 1, 1, 5), vec![1, 2, 3, 4, 5]).unwrap(),
        -1
    );
    // 2x1x3x1 -> rgb
    check_image_array!(
        Image,
        u8,
        ndarray::Array::from_shape_vec((2, 1, 3, 1), vec![1, 2, 3, 4, 5, 6]).unwrap(),
        2
    );
}
