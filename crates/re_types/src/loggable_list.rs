use crate::{
    Component, ComponentName, Datatype, DatatypeName, Loggable, ResultExt as _, SerializationResult,
};

#[allow(unused_imports)] // used in docstrings
use crate::Archetype;

// ---

/// A [`LoggableList`] represents an array's worth of [`Loggable`] instances, ready to be
/// serialized.
///
/// [`LoggableList`] is carefully designed to be erasable ("object-safe"), so that it is possible
/// to build heterogeneous collections of [`LoggableList`]s (e.g. `Vec<dyn LoggableList>`).
/// This erasability is what makes extending [`Archetype`]s possible with little effort.
///
/// You should almost never need to implement [`LoggableList`] manually, as it is already
/// blanket implemented for most common use cases (arrays/vectors/slices of loggables, etc).
pub trait LoggableList {
    type Name;

    // NOTE: It'd be tempting to have the following associated type, but that'd be
    // counterproductive, the whole point of this is to allow for heterogeneous collections!
    // type Loggable: Loggable;

    /// The fully-qualified name of this list, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name;

    /// The number of component instances stored into this list.
    fn num_instances(&self) -> usize;

    /// The underlying [`arrow2::datatypes::Field`], including datatype extensions.
    fn arrow_field(&self) -> arrow2::datatypes::Field;

    /// Serializes the list into an Arrow array.
    ///
    /// This will _never_ fail for Rerun's built-in [`LoggableList`].
    /// For the non-fallible version, see [`LoggableList::to_arrow`].
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>>;

    /// Serializes the list into an Arrow array.
    ///
    /// Panics on failure.
    /// This will _never_ fail for Rerun's built-in [`LoggableList`]s.
    ///
    /// For the fallible version, see [`LoggableList::try_to_arrow`].
    fn to_arrow(&self) -> Box<dyn ::arrow2::array::Array> {
        self.try_to_arrow().detailed_unwrap()
    }
}

/// A [`DatatypeList`] represents an array's worth of [`Datatype`] instances.
///
/// Any [`LoggableList`] with a [`Loggable::Name`] set to [`DatatypeName`] automatically
/// implements [`DatatypeList`].
pub trait DatatypeList: LoggableList<Name = DatatypeName> {}

/// A [`ComponentList`] represents an array's worth of [`Component`] instances.
///
/// Any [`LoggableList`] with a [`Loggable::Name`] set to [`ComponentName`] automatically
/// implements [`ComponentList`].
pub trait ComponentList: LoggableList<Name = ComponentName> {}

/// Holds either an owned [`ComponentList`] that lives on heap, or a reference to one.
///
/// This doesn't use [`std::borrow::Cow`] on purpose: `Cow` requires `Clone`, which would break
/// object-safety, which would prevent us from erasing [`ComponentList`]s in the first place.
pub enum AnyComponentList<'a> {
    Owned(Box<dyn ComponentList>),
    Ref(&'a dyn ComponentList),
}

impl<'a> From<&'a dyn ComponentList> for AnyComponentList<'a> {
    #[inline]
    fn from(comp_list: &'a dyn ComponentList) -> Self {
        Self::Ref(comp_list)
    }
}

impl From<Box<dyn ComponentList>> for AnyComponentList<'_> {
    #[inline]
    fn from(comp_list: Box<dyn ComponentList>) -> Self {
        Self::Owned(comp_list)
    }
}

impl<'a> AnyComponentList<'a> {
    /// Returns a reference to the inner [`ComponentList`], no matter where it lives.
    ///
    /// This doesn't use [`std::ops::Deref`] on purpose: it's associated `Target` type is not
    /// generic over lifetimes, which we need in this case.
    #[inline]
    pub fn as_list(&'a self) -> &dyn ComponentList {
        match self {
            AnyComponentList::Owned(this) => &**this,
            AnyComponentList::Ref(this) => *this,
        }
    }
}

// NOTE: Cannot do this since `Deref::Target` is not generic over lifetimes.
// impl<'a> std::ops::Deref for AnyComponentList<'a> {
//     type Target = dyn ComponentList;
//
//     fn deref(&self) -> &Self::Target {
//         match self {
//             AnyComponentList::Owned(this) => &**this,
//             AnyComponentList::Ref(this) => *this,
//         }
//     }
// }

// --- Unary ---

impl<L: Clone + Loggable> LoggableList for L {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow([std::borrow::Cow::Borrowed(self)])
    }
}

impl<D: Datatype> DatatypeList for D {}

impl<C: Component> ComponentList for C {}

// --- Vec ---

impl<L: Clone + Loggable> LoggableList for Vec<L> {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.len()
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<D: Datatype> DatatypeList for Vec<D> {}

impl<C: Component> ComponentList for Vec<C> {}

// --- Vec<Option> ---

impl<L: Loggable> LoggableList for Vec<Option<L>> {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.len()
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<D: Datatype> DatatypeList for Vec<Option<D>> {}

impl<C: Component> ComponentList for Vec<Option<C>> {}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableList for [L; N] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        N
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<D: Datatype, const N: usize> DatatypeList for [D; N] {}

impl<C: Component, const N: usize> ComponentList for [C; N] {}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableList for [Option<L>; N] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        N
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<D: Datatype, const N: usize> DatatypeList for [Option<D>; N] {}

impl<C: Component, const N: usize> ComponentList for [Option<C>; N] {}

// --- Slice ---

impl<'a, L: Loggable> LoggableList for &'a [L] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.len()
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, D: Datatype> DatatypeList for &'a [D] {}

impl<'a, C: Component> ComponentList for &'a [C] {}

// --- Slice<Option> ---

impl<'a, L: Loggable> LoggableList for &'a [Option<L>] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.len()
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, D: Datatype> DatatypeList for &'a [Option<D>] {}

impl<'a, C: Component> ComponentList for &'a [Option<C>] {}

// --- ArrayRef ---

impl<'a, L: Loggable, const N: usize> LoggableList for &'a [L; N] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        N
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeList for &'a [D; N] {}

impl<'a, C: Component, const N: usize> ComponentList for &'a [C; N] {}

// --- ArrayRef<Option> ---

impl<'a, L: Loggable, const N: usize> LoggableList for &'a [Option<L>; N] {
    type Name = L::Name;

    #[inline]
    fn name(&self) -> Self::Name {
        L::name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        N
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        L::arrow_field()
    }

    #[inline]
    fn try_to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::try_to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeList for &'a [Option<D>; N] {}

impl<'a, C: Component, const N: usize> ComponentList for &'a [Option<C>; N] {}
