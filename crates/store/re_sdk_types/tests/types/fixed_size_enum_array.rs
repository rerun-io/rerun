use re_sdk_types::Loggable as _;
use re_sdk_types::testing::datatypes::{EnumTest, FixedSizeEnumArray};

#[test]
fn roundtrip() {
    let values = FixedSizeEnumArray([EnumTest::Right, EnumTest::Down, EnumTest::Forward]);

    let arrow = FixedSizeEnumArray::to_arrow([values]).unwrap();
    let roundtrip = FixedSizeEnumArray::from_arrow(&*arrow).unwrap();

    similar_asserts::assert_eq!(vec![values], roundtrip);
}
