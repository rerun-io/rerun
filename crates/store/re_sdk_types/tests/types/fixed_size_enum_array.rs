use re_sdk_types::Loggable as _;
use re_sdk_types::testing::datatypes::{EnumTest, FixedSizeEnumArray};

#[test]
fn roundtrip() {
    let values = FixedSizeEnumArray([EnumTest::Right, EnumTest::Down, EnumTest::Forward]);

    let arrow = FixedSizeEnumArray::to_arrow_opt([Some(values)]).unwrap();
    let roundtrip = FixedSizeEnumArray::from_arrow_opt(&*arrow).unwrap();

    similar_asserts::assert_eq!(vec![Some(values)], roundtrip);
}
