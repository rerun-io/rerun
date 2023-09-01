use re_types::{
    components::{self, Point2D},
    DeserializationError, Loggable,
};

#[test]
fn validity_checks() {
    let good_non_nullable = vec![
        components::Point2D::new(1.0, 2.0), //
        components::Point2D::new(3.0, 4.0), //
    ];

    let serialized = Point2D::try_to_arrow(good_non_nullable).unwrap();
    let deserialized = Point2D::try_from_arrow(serialized.as_ref());
    assert!(deserialized.is_ok());

    let good_nullable = vec![
        Some(components::Point2D::new(1.0, 2.0)), //
        Some(components::Point2D::new(3.0, 4.0)), //
    ];

    let serialized = Point2D::try_to_arrow_opt(good_nullable).unwrap();
    let deserialized = Point2D::try_from_arrow(serialized.as_ref());
    assert!(deserialized.is_ok());

    let bad = vec![
        Some(components::Point2D::new(1.0, 2.0)), //
        None,
    ];

    let serialized = Point2D::try_to_arrow_opt(bad).unwrap();
    let deserialized = Point2D::try_from_arrow(serialized.as_ref());
    assert!(deserialized.is_err());
    assert!(matches!(
        deserialized.err().unwrap(),
        DeserializationError::MissingData { .. }
    ));
}
