use re_types::{components::Tensor, datatypes::TensorData, Loggable};

#[test]
fn tensor_roundtrip() {
    let t = vec![Tensor(TensorData::U8(vec![1, 2, 3, 4]))];

    let serialized = Tensor::try_to_arrow(t.clone(), None).unwrap();

    let deserialized = Tensor::try_from_arrow(serialized.as_ref()).unwrap();

    similar_asserts::assert_eq!(t, deserialized);
}

mod util;
