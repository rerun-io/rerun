use arrow2::datatypes::Field;
use arrow2_convert::field::ArrowField;

use re_types::ComponentName;

// ---

/// A type that can used as a Component of an Entity.
///
/// Examples of components include positions and colors.
pub trait LegacyComponent: ArrowField {
    /// The name of the component.
    fn legacy_name() -> ComponentName;

    /// Create a [`Field`] for this [`LegacyComponent`].
    fn field() -> Field {
        Field::new(Self::legacy_name().as_str(), Self::data_type(), false)
    }
}

// NOTE: We have a ton of legacy tests that rely on the old APIs and `Position2D`.
// Since the new `Position2D` is binary compatible with the old we can easily drop the old one, but
// for that we need the new one to implement the `LegacyComponent` trait.
// TODO(cmc): remove once the migration is over
impl LegacyComponent for re_types::components::Position2D {
    fn legacy_name() -> ComponentName {
        use re_types::Loggable as _;
        Self::name()
    }
}

// TODO(emilk): required to use with `range_entity_with_primary`. remove once the migration is over
impl LegacyComponent for re_types::components::Text {
    fn legacy_name() -> ComponentName {
        use re_types::Loggable as _;
        Self::name()
    }
}

// TODO(emilk): required to use with `range_entity_with_primary`. remove once the migration is over
impl LegacyComponent for re_types::components::TextLogLevel {
    fn legacy_name() -> ComponentName {
        use re_types::Loggable as _;
        Self::name()
    }
}
