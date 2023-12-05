use crate::{Component, ComponentName, Datatype, DatatypeName, Loggable, SerializationResult};

#[allow(unused_imports)] // used in docstrings
use crate::Archetype;

// ---

/// A [`LoggableBatch`] represents an array's worth of [`Loggable`] instances, ready to be
/// serialized.
///
/// [`LoggableBatch`] is carefully designed to be erasable ("object-safe"), so that it is possible
/// to build heterogeneous collections of [`LoggableBatch`]s (e.g. `Vec<dyn LoggableBatch>`).
/// This erasability is what makes extending [`Archetype`]s possible with little effort.
///
/// You should almost never need to implement [`LoggableBatch`] manually, as it is already
/// blanket implemented for most common use cases (arrays/vectors/slices of loggables, etc).
pub trait LoggableBatch {
    type Name;

    // NOTE: It'd be tempting to have the following associated type, but that'd be
    // counterproductive, the whole point of this is to allow for heterogeneous collections!
    // type Loggable: Loggable;

    /// The fully-qualified name of this batch, e.g. `rerun.datatypes.Vec2D`.
    fn name(&self) -> Self::Name;

    /// The number of component instances stored into this batch.
    fn num_instances(&self) -> usize;

    /// The underlying [`arrow2::datatypes::Field`], including datatype extensions.
    fn arrow_field(&self) -> arrow2::datatypes::Field;

    /// Serializes the batch into an Arrow array.
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>>;
}

/// A [`DatatypeBatch`] represents an array's worth of [`Datatype`] instances.
///
/// Any [`LoggableBatch`] with a [`Loggable::Name`] set to [`DatatypeName`] automatically
/// implements [`DatatypeBatch`].
pub trait DatatypeBatch: LoggableBatch<Name = DatatypeName> {}

/// A [`ComponentBatch`] represents an array's worth of [`Component`] instances.
///
/// Any [`LoggableBatch`] with a [`Loggable::Name`] set to [`ComponentName`] automatically
/// implements [`ComponentBatch`].
pub trait ComponentBatch: LoggableBatch<Name = ComponentName> {}

/// Holds either an owned [`ComponentBatch`] that lives on heap, or a reference to one.
///
/// This doesn't use [`std::borrow::Cow`] on purpose: `Cow` requires `Clone`, which would break
/// object-safety, which would prevent us from erasing [`ComponentBatch`]s in the first place.
pub enum MaybeOwnedComponentBatch<'a> {
    Owned(Box<dyn ComponentBatch>),
    Ref(&'a dyn ComponentBatch),
}

impl<'a> From<&'a dyn ComponentBatch> for MaybeOwnedComponentBatch<'a> {
    #[inline]
    fn from(comp_batch: &'a dyn ComponentBatch) -> Self {
        Self::Ref(comp_batch)
    }
}

impl From<Box<dyn ComponentBatch>> for MaybeOwnedComponentBatch<'_> {
    #[inline]
    fn from(comp_batch: Box<dyn ComponentBatch>) -> Self {
        Self::Owned(comp_batch)
    }
}

impl<'a> AsRef<dyn ComponentBatch + 'a> for MaybeOwnedComponentBatch<'a> {
    fn as_ref(&self) -> &(dyn ComponentBatch + 'a) {
        match self {
            MaybeOwnedComponentBatch::Owned(this) => &**this,
            MaybeOwnedComponentBatch::Ref(this) => *this,
        }
    }
}

impl<'a> std::ops::Deref for MaybeOwnedComponentBatch<'a> {
    type Target = dyn ComponentBatch + 'a;

    #[inline]
    fn deref(&self) -> &(dyn ComponentBatch + 'a) {
        match self {
            MaybeOwnedComponentBatch::Owned(this) => &**this,
            MaybeOwnedComponentBatch::Ref(this) => *this,
        }
    }
}

impl<'a> LoggableBatch for MaybeOwnedComponentBatch<'a> {
    type Name = ComponentName;

    #[inline]
    fn name(&self) -> Self::Name {
        self.as_ref().name()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.as_ref().num_instances()
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        self.as_ref().arrow_field()
    }

    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        self.as_ref().to_arrow()
    }
}

impl<'a> ComponentBatch for MaybeOwnedComponentBatch<'a> {}

// --- Unary ---

impl<L: Clone + Loggable> LoggableBatch for L {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow([std::borrow::Cow::Borrowed(self)])
    }
}

impl<D: Datatype> DatatypeBatch for D {}

impl<C: Component> ComponentBatch for C {}

// --- Vec ---

impl<L: Clone + Loggable> LoggableBatch for Vec<L> {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<D: Datatype> DatatypeBatch for Vec<D> {}

impl<C: Component> ComponentBatch for Vec<C> {}

// --- Vec<Option> ---

impl<L: Loggable> LoggableBatch for Vec<Option<L>> {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<D: Datatype> DatatypeBatch for Vec<Option<D>> {}

impl<C: Component> ComponentBatch for Vec<Option<C>> {}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableBatch for [L; N] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<D: Datatype, const N: usize> DatatypeBatch for [D; N] {}

impl<C: Component, const N: usize> ComponentBatch for [C; N] {}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableBatch for [Option<L>; N] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<D: Datatype, const N: usize> DatatypeBatch for [Option<D>; N] {}

impl<C: Component, const N: usize> ComponentBatch for [Option<C>; N] {}

// --- Slice ---

impl<'a, L: Loggable> LoggableBatch for &'a [L] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, D: Datatype> DatatypeBatch for &'a [D] {}

impl<'a, C: Component> ComponentBatch for &'a [C] {}

// --- Slice<Option> ---

impl<'a, L: Loggable> LoggableBatch for &'a [Option<L>] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, D: Datatype> DatatypeBatch for &'a [Option<D>] {}

impl<'a, C: Component> ComponentBatch for &'a [Option<C>] {}

// --- ArrayRef ---

impl<'a, L: Loggable, const N: usize> LoggableBatch for &'a [L; N] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeBatch for &'a [D; N] {}

impl<'a, C: Component, const N: usize> ComponentBatch for &'a [C; N] {}

// --- ArrayRef<Option> ---

impl<'a, L: Loggable, const N: usize> LoggableBatch for &'a [Option<L>; N] {
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
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, D: Datatype, const N: usize> DatatypeBatch for &'a [Option<D>; N] {}

impl<'a, C: Component, const N: usize> ComponentBatch for &'a [Option<C>; N] {}
