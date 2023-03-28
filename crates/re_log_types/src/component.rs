use arrow2::datatypes::Field;
use arrow2_convert::{
    deserialize::{ArrowArray, ArrowDeserialize},
    field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::ComponentName;

// ---

/// A type that can used as a Component of an Entity.
///
/// Examples of components include positions and colors.
pub trait Component: ArrowField {
    /// The name of the component.
    fn name() -> ComponentName;

    /// Create a [`Field`] for this [`Component`].
    fn field() -> Field {
        Field::new(Self::name().as_str(), Self::data_type(), false)
    }
}

// TODO(#1694): do a pass over these traits, this is incomprehensible... also why would a component
// ever not be (de)serializable? Do keep in mind the whole (component, datatype) story though.

/// A [`Component`] that fulfils all the conditions required to be serialized as an Arrow payload.
pub trait SerializableComponent<ArrowFieldType = Self>
where
    Self: Component + ArrowSerialize + ArrowField<Type = Self> + 'static,
{
}

impl<C> SerializableComponent for C where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static
{
}

/// A [`Component`] that fulfils all the conditions required to be deserialized from an Arrow
/// payload.
///
/// Note that due to the use of HRTBs in `arrow2_convert` traits, you will still need an extra HRTB
/// clause when marking a type as `DeserializableComponent`:
/// ```ignore
/// where
///     T: SerializableComponent,
///     for<'a> &'a T::ArrayType: IntoIterator,
/// ```
pub trait DeserializableComponent<ArrowFieldType = Self>
where
    Self: Component,
    Self: ArrowDeserialize + ArrowField<Type = ArrowFieldType> + 'static,
    Self::ArrayType: ArrowArray,
    for<'b> &'b Self::ArrayType: IntoIterator,
{
}

impl<C> DeserializableComponent for C
where
    C: Component,
    C: ArrowDeserialize + ArrowField<Type = C> + 'static,
    C::ArrayType: ArrowArray,
    for<'b> &'b C::ArrayType: IntoIterator,
{
}
