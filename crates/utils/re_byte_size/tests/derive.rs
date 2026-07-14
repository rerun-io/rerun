#![expect(clippy::assertions_on_constants)] // We use these for the test.

// `re_byte_size` re-exports our derive macro behind its (default) `derive` feature, so a single
// import brings both the trait and the macro into scope — exactly how downstream crates use it.
use re_byte_size::SizeBytes;

#[derive(SizeBytes)]
struct Pod {
    x: u32,
    y: f32,
}

#[derive(SizeBytes)]
struct Named {
    a: Vec<u8>,
    b: String,
}

#[derive(SizeBytes)]
struct Tuple(Vec<u8>, u32);

#[derive(SizeBytes)]
struct Unit;

#[derive(SizeBytes)]
struct WithIgnored {
    keep: Vec<u8>,

    #[size_bytes(ignore)]
    skip: Vec<u8>,
}

#[derive(SizeBytes)]
struct Outer {
    inner: Pod,
    name: String,
}

#[derive(SizeBytes)]
struct Generic<T> {
    items: Vec<T>,
}

#[derive(SizeBytes)]
enum MyEnum {
    Unit,
    Tuple(Vec<u8>, u32),
    Named { data: String },
    Ignoring(#[size_bytes(ignore)] Vec<u8>, u32),
}

// `crate_root` names the `re_byte_size` crate directly, sidestepping the `Cargo.toml` lookup.
#[derive(SizeBytes)]
#[size_bytes(crate_root = re_byte_size)]
struct WithCrateRoot {
    a: Vec<u8>,
}

#[test]
fn pod_struct_has_no_heap() {
    assert!(Pod::IS_POD);
    assert_eq!(Pod { x: 1, y: 2.0 }.heap_size_bytes(), 0);
}

#[test]
fn unit_struct_has_no_heap() {
    assert!(Unit::IS_POD);
    assert_eq!(Unit.heap_size_bytes(), 0);
}

#[test]
fn named_struct_sums_its_fields() {
    let value = Named {
        a: vec![1, 2, 3],
        b: "hello".to_owned(),
    };
    assert!(!Named::IS_POD);
    assert_eq!(
        value.heap_size_bytes(),
        value.a.heap_size_bytes() + value.b.heap_size_bytes()
    );
}

#[test]
fn tuple_struct_sums_its_fields() {
    let value = Tuple(vec![1, 2, 3], 7);
    assert!(!Tuple::IS_POD);
    assert_eq!(value.heap_size_bytes(), value.0.heap_size_bytes());
}

#[test]
fn ignored_field_is_left_out() {
    let value = WithIgnored {
        keep: vec![1, 2, 3],
        skip: vec![4, 5, 6, 7],
    };
    assert_eq!(value.heap_size_bytes(), value.keep.heap_size_bytes());
}

#[test]
fn pod_ness_propagates_through_nesting() {
    // `Pod` is POD, so `Outer` is POD exactly when `name` would be — it isn't.
    assert!(!Outer::IS_POD);
    let value = Outer {
        inner: Pod { x: 1, y: 2.0 },
        name: "hello".to_owned(),
    };
    assert_eq!(value.heap_size_bytes(), value.name.heap_size_bytes());
}

#[test]
fn generic_struct_sums_its_fields() {
    let value = Generic {
        items: vec![1u32, 2, 3],
    };
    assert!(!Generic::<u32>::IS_POD);
    assert_eq!(value.heap_size_bytes(), value.items.heap_size_bytes());
}

#[test]
fn enum_sizes_the_active_variant() {
    assert!(!MyEnum::IS_POD);

    assert_eq!(MyEnum::Unit.heap_size_bytes(), 0);

    let data = vec![1u8, 2, 3];
    assert_eq!(
        MyEnum::Tuple(data.clone(), 7).heap_size_bytes(),
        data.heap_size_bytes()
    );

    let text = "hello".to_owned();
    assert_eq!(
        MyEnum::Named { data: text.clone() }.heap_size_bytes(),
        text.heap_size_bytes()
    );

    // The first field is ignored, the second is POD, so the variant has no heap.
    assert_eq!(MyEnum::Ignoring(vec![1, 2, 3], 9).heap_size_bytes(), 0);
}

#[test]
fn crate_root_override_sizes_its_fields() {
    let value = WithCrateRoot { a: vec![1, 2, 3] };
    assert!(!WithCrateRoot::IS_POD);
    assert_eq!(value.heap_size_bytes(), value.a.heap_size_bytes());
}
