#[cfg(feature = "polars")]
#[test]
fn test_transform_to_polars() {
    use re_log_types::{field_types::Quaternion, field_types::Vec3D, Pinhole, Rigid3, Transform};

    let transforms = vec![
        Some(Transform::Pinhole(Pinhole {
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            resolution: None,
        })),
        Some(Transform::Rigid3(Rigid3 {
            rotation: Quaternion {
                x: 11.0,
                y: 12.0,
                z: 13.0,
                w: 14.0,
            },
            translation: Vec3D {
                x: 15.0,
                y: 16.0,
                z: 17.0,
            },
        })),
        Some(Transform::Pinhole(Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]],
            resolution: Some([123.0, 456.0]),
        })),
    ];

    let df = re_query::dataframe_util::df_builder1(&transforms);

    assert!(df.is_ok());
}
