use crate::{
    Component, ComponentList, Datatype, DatatypeList, Loggable, LoggableList, SerializationResult,
};

// --- Unary ---

impl<L: Clone + Loggable> LoggableList for L {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow([std::borrow::Cow::Borrowed(self)], None)
    }
}

impl<D: Datatype> DatatypeList for D {}

impl<C: Component> ComponentList for C {}

// --- Vec ---

impl<L: Clone + Loggable> LoggableList for Vec<L> {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)), None)
    }
}

impl<D: Datatype> DatatypeList for Vec<D> {}

impl<C: Component> ComponentList for Vec<C> {}

// --- Vec<Option> ---

impl<L: Loggable> LoggableList for Vec<Option<L>> {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
            None,
        )
    }
}

impl<D: Datatype> DatatypeList for Vec<Option<D>> {}

impl<C: Component> ComponentList for Vec<Option<C>> {}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableList for [L; N] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)), None)
    }
}

impl<D: Datatype, const N: usize> DatatypeList for [D; N] {}

impl<C: Component, const N: usize> ComponentList for [C; N] {}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableList for [Option<L>; N] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
            None,
        )
    }
}

impl<D: Datatype, const N: usize> DatatypeList for [Option<D>; N] {}

impl<C: Component, const N: usize> ComponentList for [Option<C>; N] {}

// --- Slice ---

impl<'a, L: Loggable> LoggableList for &'a [L] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)), None)
    }
}

impl<'a, D: Datatype> DatatypeList for &'a [D] {}

impl<'a, C: Component> ComponentList for &'a [C] {}

// --- Slice<Option> ---

impl<'a, L: Loggable> LoggableList for &'a [Option<L>] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
            None,
        )
    }
}

impl<'a, D: Datatype> DatatypeList for &'a [Option<D>] {}

impl<'a, C: Component> ComponentList for &'a [Option<C>] {}

// --- ArrayRef ---

impl<'a, L: Loggable, const N: usize> LoggableList for &'a [L; N] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)), None)
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeList for &'a [D; N] {}

impl<'a, C: Component, const N: usize> ComponentList for &'a [C; N] {}

// --- ArrayRef<Option> ---

impl<'a, L: Loggable, const N: usize> LoggableList for &'a [Option<L>; N] {
    type Name = L::Name;

    /// The fully-qualified name of this loggable, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name {
        L::name()
    }

    /// The underlying [`arrow2::datatypes::Field`].
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
            None,
        )
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeList for &'a [Option<D>; N] {}

impl<'a, C: Component, const N: usize> ComponentList for &'a [Option<C>; N] {}
