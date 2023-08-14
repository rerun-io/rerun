use re_types::{
    components::Tensor,
    datatypes::{self, TensorData, TensorDimension, TensorId},
    Loggable,
};

#[test]
fn tensor_roundtrip() {
    let t = vec![Tensor(datatypes::Tensor {
        id: TensorId {
            id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        },
        shape: vec![
            TensorDimension {
                size: 2,
                name: None,
            },
            TensorDimension {
                size: 2,
                name: None,
            },
        ],
        data: TensorData::U8(vec![1, 2, 3, 4, 5, 6].into()),
    })];

    let serialized = Tensor::try_to_arrow(t.clone(), None).unwrap();

    let deserialized = Tensor::try_from_arrow(serialized.as_ref()).unwrap();

    similar_asserts::assert_eq!(t, deserialized);
}

mod util;
