use std::collections::HashMap;

use re_types::{
    archetypes::SegmentationImage,
    datatypes::{TensorBuffer, TensorData, TensorDimension},
    Archetype as _, AsComponents as _,
};

mod util;

#[test]
fn segmentation_image_roundtrip() {
    let all_expected = [SegmentationImage {
        data: TensorData {
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
        draw_order: None,
    }];

    let all_arch_serialized = [SegmentationImage::try_from(ndarray::array![
        [1u8, 2, 3],
        [4, 5, 6]
    ])
    .unwrap()
    .to_arrow().unwrap()];

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

        let deserialized = SegmentationImage::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
