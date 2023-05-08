#[cfg(feature = "polars")]
#[test]
fn test_transform_to_polars() {
    use re_log_types::component_types::{
        Pinhole, Quaternion, Transform3D, TranslationRotationScale,
    };

    let transforms = vec![
        Some(Transform3D::Pinhole(Pinhole {
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
            resolution: None,
        })),
        Some(Transform3D::Affine3D(
            TranslationRotationScale {
                rotation: Quaternion {
                    x: 11.0,
                    y: 12.0,
                    z: 13.0,
                    w: 14.0,
                }
                .into(),
                translation: [15.0, 16.0, 17.0].into(),
                scale: [18.0, 19.0, 20.0].into(),
            }
            .into(),
        )),
        Some(Transform3D::Pinhole(Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]].into(),
            resolution: Some([123.0, 456.0].into()),
        })),
    ];

    let df = re_query::dataframe_util::df_builder1(&transforms);

    assert!(df.is_ok());
}
