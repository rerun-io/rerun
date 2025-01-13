use std::borrow::Cow;

use crate::{
    ArchetypeFieldName, ArchetypeName, Component, ComponentDescriptor, ComponentName, Loggable,
    SerializationResult,
};

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

#[allow(dead_code)]
fn assert_loggablebatch_object_safe() {
    let _: &dyn LoggableBatch;
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
    //
    // TODO(cmc): This should probably go away, but we'll see about that once I start tackling
    // partial updates themselves.
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

    /// Serializes the contents of this [`ComponentBatch`].
    ///
    /// Once serialized, the data is ready to be logged into Rerun via the [`AsComponents`] trait.
    ///
    /// # Fallibility
    ///
    /// There are very few ways in which serialization can fail, all of which are very rare to hit
    /// in practice.
    /// One such example is trying to serialize data with more than 2^31 elements into a `ListArray`.
    ///
    /// For that reason, this method favors a nice user experience over error handling: errors will
    /// merely be logged, not returned (except in debug builds, where all errors panic).
    ///
    /// See also [`ComponentBatch::try_serialized`].
    ///
    /// [`AsComponents`]: [crate::AsComponents]
    #[inline]
    fn serialized(&self) -> Option<SerializedComponentBatch> {
        match self.try_serialized() {
            Ok(array) => Some(array),

            #[cfg(debug_assertions)]
            Err(err) => {
                panic!(
                    "failed to serialize data for {}: {}",
                    self.descriptor(),
                    re_error::format_ref(&err)
                )
            }

            #[cfg(not(debug_assertions))]
            Err(err) => {
                re_log::error!(
                    descriptor = %self.descriptor(),
                    "failed to serialize data: {}",
                    re_error::format_ref(&err)
                );
                None
            }
        }
    }

    /// Serializes the contents of this [`ComponentBatch`].
    ///
    /// Once serialized, the data is ready to be logged into Rerun via the [`AsComponents`] trait.
    ///
    /// # Fallibility
    ///
    /// There are very few ways in which serialization can fail, all of which are very rare to hit
    /// in practice.
    ///
    /// For that reason, it generally makes sense to favor a nice user experience over error handling
    /// in most cases, see [`ComponentBatch::serialized`].
    ///
    /// [`AsComponents`]: [crate::AsComponents]
    #[inline]
    fn try_serialized(&self) -> SerializationResult<SerializedComponentBatch> {
        Ok(SerializedComponentBatch {
            array: self.to_arrow()?,
            descriptor: self.descriptor().into_owned(),
        })
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

#[allow(dead_code)]
fn assert_component_batch_object_safe() {
    let _: &dyn LoggableBatch;
}

/// The serialized contents of a [`ComponentBatch`] with associated [`ComponentDescriptor`].
///
/// This is what gets logged into Rerun:
/// * See [`ComponentBatch`] to easily serialize component data.
/// * See [`AsComponents`] for logging serialized data.
///
/// [`AsComponents`]: [crate::AsComponents]
#[derive(Debug, Clone)]
pub struct SerializedComponentBatch {
    pub array: arrow::array::ArrayRef,

    // TODO(cmc): Maybe Cow<> this one if it grows bigger. Or intern descriptors altogether, most likely.
    pub descriptor: ComponentDescriptor,
}

impl re_byte_size::SizeBytes for SerializedComponentBatch {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { array, descriptor } = self;
        array.heap_size_bytes() + descriptor.heap_size_bytes()
    }
}

impl PartialEq for SerializedComponentBatch {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self { array, descriptor } = self;

        // Descriptor first!
        *descriptor == other.descriptor && **array == *other.array
    }
}

impl SerializedComponentBatch {
    #[inline]
    pub fn new(array: arrow::array::ArrayRef, descriptor: ComponentDescriptor) -> Self {
        Self { array, descriptor }
    }

    #[inline]
    pub fn with_descriptor_override(self, descriptor: ComponentDescriptor) -> Self {
        Self { descriptor, ..self }
    }

    /// Unconditionally sets the descriptor's `archetype_name` to the given one.
    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        self.descriptor = self.descriptor.with_archetype_name(archetype_name);
        self
    }

    /// Unconditionally sets the descriptor's `archetype_field_name` to the given one.
    #[inline]
    pub fn with_archetype_field_name(mut self, archetype_field_name: ArchetypeFieldName) -> Self {
        self.descriptor = self
            .descriptor
            .with_archetype_field_name(archetype_field_name);
        self
    }

    /// Sets the descriptor's `archetype_name` to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype_name(mut self, archetype_name: impl Fn() -> ArchetypeName) -> Self {
        self.descriptor = self.descriptor.or_with_archetype_name(archetype_name);
        self
    }

    /// Sets the descriptor's `archetype_field_name` to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype_field_name(
        mut self,
        archetype_field_name: impl FnOnce() -> ArchetypeFieldName,
    ) -> Self {
        self.descriptor = self
            .descriptor
            .or_with_archetype_field_name(archetype_field_name);
        self
    }
}

// TODO(cmc): All these crazy types are about to disappear. ComponentBatch should only live at the
// edge, and therefore not require all these crazy kinds of derivatives (require eager serialization).

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

impl<L: Loggable> LoggableBatch for [L] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for [C] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}

// --- Slice<Option> ---

impl<L: Loggable> LoggableBatch for [Option<L>] {
    #[inline]
    fn to_arrow2(&self) -> SerializationResult<Box<dyn ::arrow2::array::Array>> {
        L::to_arrow2_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for [Option<C>] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        C::descriptor().into()
    }
}
