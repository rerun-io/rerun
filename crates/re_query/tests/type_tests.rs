#[cfg(feature = "polars")]
#[test]
fn test_transform_to_polars() {
    use re_types::components::Transform3D;
    use re_types::datatypes::{Quaternion, TranslationRotationScale3D};

    let transforms = vec![Some(Transform3D::from(TranslationRotationScale3D {
        rotation: Some(Quaternion::from_xyzw([11.0, 12.0, 13.0, 14.0]).into()),
        translation: Some([15.0, 16.0, 17.0].into()),
        scale: Some([18.0, 19.0, 20.0].into()),
        from_parent: true,
    }))];

    let df = re_query::dataframe_util::df_builder1(&transforms);

    assert!(df.is_ok());
}
