#[test]
fn test_space_view_maximized() {
    use super::SpaceViewMaximized;

    use re_types::Loggable as _;
    use re_viewer_context::SpaceViewId;

    for data in [
        [SpaceViewMaximized(None)],
        [SpaceViewMaximized(Some(SpaceViewId::random().into()))],
    ] {
        let array: Box<dyn arrow2::array::Array> =
            SpaceViewMaximized::to_arrow(data.as_slice()).unwrap();
        let ret: Vec<SpaceViewMaximized> = SpaceViewMaximized::from_arrow(array.as_ref()).unwrap();
        assert_eq!(&data, ret.as_slice());
    }
}
