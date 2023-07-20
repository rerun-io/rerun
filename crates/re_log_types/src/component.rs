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

    /// Create a [`Field`] for this [`Component`].
    fn field() -> Field {
        Field::new(Self::legacy_name().as_str(), Self::data_type(), false)
    }
}
