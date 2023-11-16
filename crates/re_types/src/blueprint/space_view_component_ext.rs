use super::SpaceViewComponent;

impl std::fmt::Debug for SpaceViewComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SpaceViewComponent")
    }
}

#[test]
fn test_space_view_component() {
    use re_types::Loggable as _;

    let space_view = crate::SpaceViewBlueprint::new(
        "Spatial".into(),
        &"foo".into(),
        std::iter::once(&"foo/bar".into()),
    );

    let data = [SpaceViewComponent { space_view }];
    let array: Box<dyn arrow2::array::Array> =
        SpaceViewComponent::to_arrow(data.as_slice()).unwrap();
    let ret: Vec<SpaceViewComponent> = SpaceViewComponent::from_arrow(array.as_ref()).unwrap();
    assert_eq!(data.len(), ret.len());
    assert!(data
        .iter()
        .zip(ret)
        .all(|(l, r)| !l.space_view.has_edits(&r.space_view)));
}
