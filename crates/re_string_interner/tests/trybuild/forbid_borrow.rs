use ahash::HashMap;

use re_string_interner::*;

#[allow(clippy::redundant_type_annotations)]
fn main() {
    declare_new_type!(
        /// My typesafe string
        pub struct MyString;
    );

    let hello_my_string: MyString = "hello".into();
    let hello_str: &str = "hello";

    let mut my_map: HashMap<MyString, u32> = HashMap::default();
    my_map.insert(hello_my_string, 42u32);

    assert_eq!(Some(42), my_map.get(&hello_my_string).copied());
    assert_eq!(Some(42), my_map.get(hello_str).copied());

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/forbid_borrow.rs");
}
