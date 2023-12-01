use super::ViewportLayout;

impl Default for ViewportLayout {
    fn default() -> Self {
        Self {
            space_view_keys: Default::default(),
            tree: egui_tiles::Tree::empty("viewport_tree"),
            auto_layout: true,
        }
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
