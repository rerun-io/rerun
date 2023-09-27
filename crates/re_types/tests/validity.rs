use re_types::{
    components::{self, Position2D},
    DeserializationError, Loggable,
};

#[test]
fn validity_checks() {
    let good_non_nullable = vec![
        components::Position2D::new(1.0, 2.0), //
        components::Position2D::new(3.0, 4.0), //
    ];

    let serialized = Position2D::to_arrow(good_non_nullable).unwrap();
    let deserialized = Position2D::from_arrow(serialized.as_ref());
    assert!(deserialized.is_ok());

    let good_nullable = vec![
        Some(components::Position2D::new(1.0, 2.0)), //
        Some(components::Position2D::new(3.0, 4.0)), //
    ];

    let serialized = Position2D::to_arrow_opt(good_nullable).unwrap();
    let deserialized = Position2D::from_arrow(serialized.as_ref());
    assert!(deserialized.is_ok());

    let bad = vec![
        Some(components::Position2D::new(1.0, 2.0)), //
        None,
    ];

    let serialized = Position2D::to_arrow_opt(bad).unwrap();
    let deserialized = Position2D::from_arrow(serialized.as_ref());
    assert!(deserialized.is_err());
    assert!(matches!(
        deserialized.err().unwrap(),
        DeserializationError::MissingData { .. }
    ));
}
