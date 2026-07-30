use re_sdk_types::components::{self, Position2D};
use re_sdk_types::{DeserializationError, Loggable as _};

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
    let actual_error = deserialized.err().unwrap().without_context();
    assert!(
        matches!(actual_error, DeserializationError::MissingData { .. }),
        "Expected error MissingData, got {actual_error:?}",
    );
}

/// The buffer-slice fast path used by fixed-size-list deserializers must reject malformed data
/// rather than panic inside `bytemuck`.
///
/// Deserializers run on arbitrary `.rrd` payloads, so a wrong list width is untrusted input,
/// not a programming error.
#[test]
fn nested_fixed_size_list_of_the_wrong_width_errors() {
    use std::sync::Arc;

    use arrow::array::{Array as _, FixedSizeListArray, Float16Array};
    use arrow::datatypes::{DataType, Field};
    use re_sdk_types::datatypes::SphericalHarmonics3Rgb;

    // `SphericalHarmonics3Rgb` is a `FixedSizeList(FixedSizeList(f16, 3), 15)`.
    // Widen the inner list to 4: still 60 values, but no longer a whole number of coefficients.
    let values = Float16Array::from(vec![half::f16::ZERO; 60]);
    let inner_field = Arc::new(Field::new("item", DataType::Float16, false));
    let inner = FixedSizeListArray::new(inner_field.clone(), 4, Arc::new(values), None);
    let outer_field = Arc::new(Field::new("item", inner.data_type().clone(), false));
    let outer = FixedSizeListArray::new(outer_field, 15, Arc::new(inner), None);

    let deserialized = SphericalHarmonics3Rgb::from_arrow(&outer);
    assert!(
        deserialized.is_err(),
        "Expected a deserialization error, got {deserialized:?}"
    );
}
