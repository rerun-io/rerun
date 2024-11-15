use crate::{Component, ComponentName, Loggable, SerializationResult};

use arrow2::array::ListArray as ArrowListArray;

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
    // NOTE: It'd be tempting to have the following associated type, but that'd be
    // counterproductive, the whole point of this is to allow for heterogeneous collections!
    // type Loggable: Loggable;

    /// Serializes the batch into an Arrow array.
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>>;
}

/// A [`ComponentBatch`] represents an array's worth of [`Component`] instances.
pub trait ComponentBatch: LoggableBatch {
    /// The fully-qualified name of this component batch, e.g. `rerun.components.Position2D`.
    fn name(&self) -> ComponentName;

    /// Serializes the batch into an Arrow list array with a single component per list.
    fn to_arrow_list_array(&self) -> SerializationResult<ArrowListArray<i32>> {
        let array = self.to_arrow()?;
        let offsets =
            arrow2::offset::Offsets::try_from_lengths(std::iter::repeat(1).take(array.len()))?;
        let data_type = ArrowListArray::<i32>::default_datatype(array.data_type().clone());
        ArrowListArray::<i32>::try_new(data_type, offsets.into(), array.to_boxed(), None)
            .map_err(|err| err.into())
    }
}

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
    #[inline]
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
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        self.as_ref().to_arrow()
    }
}

impl<'a> ComponentBatch for MaybeOwnedComponentBatch<'a> {
    #[inline]
    fn name(&self) -> ComponentName {
        self.as_ref().name()
    }
}

// --- Unary ---

impl<L: Clone + Loggable> LoggableBatch for L {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow([std::borrow::Cow::Borrowed(self)])
    }
}

impl<C: Component> ComponentBatch for C {
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Unary Option ---

impl<L: Clone + Loggable> LoggableBatch for Option<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Option<C> {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Vec ---

impl<L: Clone + Loggable> LoggableBatch for Vec<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Vec<C> {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Vec<Option> ---

impl<L: Loggable> LoggableBatch for Vec<Option<L>> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for Vec<Option<C>> {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableBatch for [L; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component, const N: usize> ComponentBatch for [C; N] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableBatch for [Option<L>; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component, const N: usize> ComponentBatch for [Option<C>; N] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Slice ---

impl<'a, L: Loggable> LoggableBatch for &'a [L] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, C: Component> ComponentBatch for &'a [C] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- Slice<Option> ---

impl<'a, L: Loggable> LoggableBatch for &'a [Option<L>] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, C: Component> ComponentBatch for &'a [Option<C>] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- ArrayRef ---

impl<'a, L: Loggable, const N: usize> LoggableBatch for &'a [L; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<'a, C: Component, const N: usize> ComponentBatch for &'a [C; N] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}

// --- ArrayRef<Option> ---

impl<'a, L: Loggable, const N: usize> LoggableBatch for &'a [Option<L>; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<'a, C: Component, const N: usize> ComponentBatch for &'a [Option<C>; N] {
    #[inline]
    fn name(&self) -> ComponentName {
        C::name()
    }
}
