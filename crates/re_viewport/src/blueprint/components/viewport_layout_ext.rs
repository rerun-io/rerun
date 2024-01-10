use super::ViewportLayout;

impl Default for ViewportLayout {
    fn default() -> Self {
        Self(egui_tiles::Tree::empty("viewport_tree"))
    }
}

impl re_types_core::SizeBytes for ViewportLayout {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): Implementing SizeBytes for this type would require a lot of effort,
        // which would be wasted since this is supposed to go away very soon.
        #[allow(clippy::manual_assert)] // readability
        if cfg!(debug_assertions) {
            panic!("ViewportLayout does not report its size properly");
        }

        0
    }
}

#[test]
fn test_viewport_layout() {
    use re_types::Loggable as _;

    let viewport_layout = ViewportLayout::default();

    let data = [viewport_layout];
    let array: Box<dyn arrow2::array::Array> = ViewportLayout::to_arrow(data.as_slice()).unwrap();
    let ret: Vec<ViewportLayout> = ViewportLayout::from_arrow(array.as_ref()).unwrap();
    assert_eq!(data.to_vec(), ret);
}
