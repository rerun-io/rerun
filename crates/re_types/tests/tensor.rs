use std::collections::HashMap;

use re_types::{
    archetypes::Tensor,
    datatypes::{TensorBuffer, TensorData, TensorDimension, TensorId, TensorMeaning},
    Archetype as _,
};

#[test]
fn tensor_roundtrip() {
    let all_expected = [Tensor {
        data: TensorData {
            id: TensorId {
                id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            },
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
        meaning: Some(TensorMeaning::ClassId(true).into()),
    }];

    let all_arch = [Tensor::new(TensorData {
        id: TensorId {
            id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        },
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
    })
    .with_meaning(TensorMeaning::ClassId(true))];

    let expected_extensions: HashMap<_, _> = [
        ("data", vec!["rerun.components.TensorData"]),
        ("meaning", vec!["rerun.components.TensorMeaning"]),
    ]
    .into();

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow();
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

        let deserialized = Tensor::from_arrow(serialized);
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

mod util;
