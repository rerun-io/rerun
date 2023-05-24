#[cfg(feature = "polars")]
#[test]
fn test_transform_to_polars() {
    use re_log_types::component_types::{Quaternion, Transform3D, TranslationRotationScale3D};

    let transforms = vec![Some(Transform3D::from_parent(TranslationRotationScale3D {
        rotation: Some(
            Quaternion {
                x: 11.0,
                y: 12.0,
                z: 13.0,
                w: 14.0,
            }
            .into(),
        ),
        translation: Some([15.0, 16.0, 17.0].into()),
        scale: Some([18.0, 19.0, 20.0].into()),
    }))];

    let df = re_query::dataframe_util::df_builder1(&transforms);

    assert!(df.is_ok());
}
