use std::borrow::Cow;

use crate::{Component, ComponentDescriptor, ComponentName, Loggable, SerializationResult};

use arrow2::array::ListArray as Arrow2ListArray;

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
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        self.to_arrow2().map(|array| array.into())
    }

    /// Serializes the batch into an Arrow2 array.
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>>;
}

/// A [`ComponentBatch`] represents an array's worth of [`Component`] instances.
pub trait ComponentBatch: LoggableBatch {
    /// Serializes the batch into an Arrow list array with a single component per list.
    fn to_arrow_list_array(&self) -> SerializationResult<Arrow2ListArray<i32>> {
        let array = self.to_arrow2()?;
        let offsets =
            arrow2::offset::Offsets::try_from_lengths(std::iter::repeat(1).take(array.len()))?;
        let data_type = Arrow2ListArray::<i32>::default_datatype(array.data_type().clone());
        Arrow2ListArray::<i32>::try_new(data_type, offsets.into(), array.to_boxed(), None)
            .map_err(|err| err.into())
    }

    /// Returns the complete [`ComponentDescriptor`] for this [`ComponentBatch`].
    ///
    /// Every component batch is uniquely identified by its [`ComponentDescriptor`].
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor>;

    // Wraps the current [`ComponentBatch`] with the given descriptor.
    fn with_descriptor(
        &self,
        descriptor: ComponentDescriptor,
    ) -> ComponentBatchCowWithDescriptor<'_>
    where
        Self: Sized,
    {
        ComponentBatchCowWithDescriptor::new(ComponentBatchCow::Ref(self as &dyn ComponentBatch))
            .with_descriptor_override(descriptor)
    }

    /// The fully-qualified name of this component batch, e.g. `rerun.components.Position2D`.
    ///
    /// This is a trivial but useful helper for `self.descriptor().component_name`.
    ///
    /// The default implementation already does the right thing. Do not override unless you know
    /// what you're doing.
    /// `Self::name()` must exactly match the value returned by `self.descriptor().component_name`,
    /// or undefined behavior ensues.
    #[inline]
    fn name(&self) -> ComponentName {
        self.descriptor().component_name
    }
}

/// Some [`ComponentBatch`], optionally with an overridden [`ComponentDescriptor`].
///
/// Used by implementers of [`crate::AsComponents`] to both efficiently expose their component data
/// and assign the right tags given the surrounding context.
pub struct ComponentBatchCowWithDescriptor<'a> {
    /// The component data.
    pub batch: ComponentBatchCow<'a>,

    /// If set, will override the [`ComponentBatch`]'s [`ComponentDescriptor`].
    pub descriptor_override: Option<ComponentDescriptor>,
}

impl<'a> From<ComponentBatchCow<'a>> for ComponentBatchCowWithDescriptor<'a> {
    #[inline]
    fn from(batch: ComponentBatchCow<'a>) -> Self {
        Self::new(batch)
    }
}

impl<'a> ComponentBatchCowWithDescriptor<'a> {
    #[inline]
    pub fn new(batch: impl Into<ComponentBatchCow<'a>>) -> Self {
        Self {
            batch: batch.into(),
            descriptor_override: None,
        }
    }

    #[inline]
    pub fn with_descriptor_override(self, descriptor: ComponentDescriptor) -> Self {
        Self {
            descriptor_override: Some(descriptor),
            ..self
        }
    }
}

impl LoggableBatch for ComponentBatchCowWithDescriptor<'_> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        self.batch.to_arrow2()
    }
}

impl<'a> ComponentBatch for ComponentBatchCowWithDescriptor<'a> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        self.descriptor_override
            .as_ref()
            .map(Into::into)
            .unwrap_or_else(|| self.batch.descriptor())
    }

    #[inline]
    fn name(&self) -> ComponentName {
        self.batch.name()
    }
}

/// Holds either an owned [`ComponentBatch`] that lives on heap, or a reference to one.
///
/// This doesn't use [`std::borrow::Cow`] on purpose: `Cow` requires `Clone`, which would break
/// object-safety, which would prevent us from erasing [`ComponentBatch`]s in the first place.
pub enum ComponentBatchCow<'a> {
    Owned(Box<dyn ComponentBatch>),
    Ref(&'a dyn ComponentBatch),
}

impl<'a> From<&'a dyn ComponentBatch> for ComponentBatchCow<'a> {
    #[inline]
    fn from(comp_batch: &'a dyn ComponentBatch) -> Self {
        Self::Ref(comp_batch)
    }
}

impl From<Box<dyn ComponentBatch>> for ComponentBatchCow<'_> {
    #[inline]
    fn from(comp_batch: Box<dyn ComponentBatch>) -> Self {
        Self::Owned(comp_batch)
    }
}

impl<'a> std::ops::Deref for ComponentBatchCow<'a> {
    type Target = dyn ComponentBatch + 'a;

    #[inline]
    fn deref(&self) -> &(dyn ComponentBatch + 'a) {
        match self {
            ComponentBatchCow::Owned(this) => &**this,
            ComponentBatchCow::Ref(this) => *this,
        }
    }
}

impl<'a> LoggableBatch for ComponentBatchCow<'a> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        (**self).to_arrow2()
    }
}

impl<'a> ComponentBatch for ComponentBatchCow<'a> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        (**self).descriptor()
    }
}

// --- Unary ---

impl<L: Clone + Loggable> LoggableBatch for L {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2([std::borrow::Cow::Borrowed(self)])
    }
}

impl<C: Component> ComponentBatch for C {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Unary Option ---

impl<L: Clone + Loggable> LoggableBatch for Option<L> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Option<C> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Vec ---

impl<L: Clone + Loggable> LoggableBatch for Vec<L> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Vec<C> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Vec<Option> ---

impl<L: Loggable> LoggableBatch for Vec<Option<L>> {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for Vec<Option<C>> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableBatch for [L; N] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component, const N: usize> ComponentBatch for [C; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableBatch for [Option<L>; N] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component, const N: usize> ComponentBatch for [Option<C>; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Slice ---

impl<L: Loggable> LoggableBatch for &[L] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for &[C] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Slice<Option> ---

impl<L: Loggable> LoggableBatch for &[Option<L>] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for &[Option<C>] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- ArrayRef ---

impl<L: Loggable, const N: usize> LoggableBatch for &[L; N] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component, const N: usize> ComponentBatch for &[C; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- ArrayRef<Option> ---

impl<L: Loggable, const N: usize> LoggableBatch for &[Option<L>; N] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component, const N: usize> ComponentBatch for &[Option<C>; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}
