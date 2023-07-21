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

// TODO: explain
impl LegacyComponent for re_types::components::Point2D {
    fn legacy_name() -> ComponentName {
        use re_types::Loggable as _;
        Self::name()
    }
}
