use std::borrow::Cow;

use crate::{
    ArchetypeFieldName, ArchetypeName, Component, ComponentDescriptor, ComponentName, Loggable,
    SerializationResult,
};

use arrow::array::ListArray as ArrowListArray;

#[allow(unused_imports, clippy::unused_trait_names)] // used in docstrings
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
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef>;
}

#[allow(dead_code)]
fn assert_loggablebatch_object_safe() {
    let _: &dyn LoggableBatch;
}

/// A [`ComponentBatch`] represents an array's worth of [`Component`] instances.
pub trait ComponentBatch: LoggableBatch {
    /// Serializes the batch into an Arrow list array with a single component per list.
    fn to_arrow_list_array(&self) -> SerializationResult<ArrowListArray> {
        let array = self.to_arrow()?;
        let offsets =
            arrow::buffer::OffsetBuffer::from_lengths(std::iter::repeat(1).take(array.len()));
        let nullable = true;
        let field = arrow::datatypes::Field::new("item", array.data_type().clone(), nullable);
        ArrowListArray::try_new(field.into(), offsets, array, None).map_err(|err| err.into())
    }

    /// Returns the complete [`ComponentDescriptor`] for this [`ComponentBatch`].
    ///
    /// Every component batch is uniquely identified by its [`ComponentDescriptor`].
    #[deprecated]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor>;

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
    fn serialized(&self, component_descr: ComponentDescriptor) -> Option<SerializedComponentBatch> {
        match self.try_serialized(component_descr.clone()) {
            Ok(array) => Some(array),

            #[cfg(debug_assertions)]
            Err(err) => {
                panic!(
                    "failed to serialize data for {}: {}",
                    component_descr,
                    re_error::format_ref(&err)
                )
            }

            #[cfg(not(debug_assertions))]
            Err(err) => {
                re_log::error!(
                    descriptor = %component_descr,
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
    fn try_serialized(
        &self,
        component_descr: ComponentDescriptor,
    ) -> SerializationResult<SerializedComponentBatch> {
        Ok(SerializedComponentBatch {
            array: self.to_arrow()?,
            descriptor: component_descr,
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
    #[deprecated]
    fn name(&self) -> ComponentName {
        self.descriptor().component_name
    }
}

#[allow(dead_code)]
fn assert_component_batch_object_safe() {
    let _: &dyn LoggableBatch;
}

// ---

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

/// A column's worth of component data.
///
/// If a [`SerializedComponentBatch`] represents one row's worth of data
#[derive(Debug, Clone)]
pub struct SerializedComponentColumn {
    pub list_array: arrow::array::ListArray,

    // TODO(cmc): Maybe Cow<> this one if it grows bigger. Or intern descriptors altogether, most likely.
    pub descriptor: ComponentDescriptor,
}

impl SerializedComponentColumn {
    /// Repartitions the component data into multiple sub-batches, ignoring the previous partitioning.
    ///
    /// The specified `lengths` must sum to the total length of the component batch.
    pub fn repartitioned(
        self,
        lengths: impl IntoIterator<Item = usize>,
    ) -> SerializationResult<Self> {
        let Self {
            list_array,
            descriptor,
        } = self;

        let list_array = re_arrow_util::repartition_list_array(list_array, lengths)?;

        Ok(Self {
            list_array,
            descriptor,
        })
    }
}

impl From<SerializedComponentBatch> for SerializedComponentColumn {
    #[inline]
    fn from(batch: SerializedComponentBatch) -> Self {
        use arrow::{
            array::{Array as _, ListArray},
            buffer::OffsetBuffer,
            datatypes::Field,
        };

        let list_array = {
            let nullable = true;
            let field = Field::new_list_field(batch.array.data_type().clone(), nullable);
            let offsets = OffsetBuffer::from_lengths(std::iter::once(batch.array.len()));
            let nulls = None;
            ListArray::new(field.into(), offsets, batch.array, nulls)
        };

        Self {
            list_array,
            descriptor: batch.descriptor,
        }
    }
}

impl SerializedComponentBatch {
    /// Partitions the component data into multiple sub-batches.
    ///
    /// Specifically, this transforms the existing [`SerializedComponentBatch`] data into a [`SerializedComponentColumn`].
    ///
    /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
    ///
    /// The specified `lengths` must sum to the total length of the component batch.
    #[inline]
    pub fn partitioned(
        self,
        lengths: impl IntoIterator<Item = usize>,
    ) -> SerializationResult<SerializedComponentColumn> {
        let column: SerializedComponentColumn = self.into();
        column.repartitioned(lengths)
    }
}

// ---

// TODO(cmc): This is far from ideal and feels very hackish, but for now the priority is getting
// all things related to tags up and running so we can gather learnings.
// This is only used on the archetype deserialization path, which isn't ever used outside of tests anyway.

// TODO(cmc): we really shouldn't be duplicating these.

/// The key used to identify the [`crate::ArchetypeName`] in field-level metadata.
const FIELD_METADATA_KEY_ARCHETYPE_NAME: &str = "rerun.archetype_name";

/// The key used to identify the [`crate::ArchetypeFieldName`] in field-level metadata.
const FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME: &str = "rerun.archetype_field_name";

impl From<&SerializedComponentBatch> for arrow::datatypes::Field {
    #[inline]
    fn from(batch: &SerializedComponentBatch) -> Self {
        Self::new(
            batch.descriptor.component_name.to_string(),
            batch.array.data_type().clone(),
            false,
        )
        .with_metadata(
            [
                batch.descriptor.archetype_name.map(|name| {
                    (
                        FIELD_METADATA_KEY_ARCHETYPE_NAME.to_owned(),
                        name.to_string(),
                    )
                }),
                batch.descriptor.archetype_field_name.map(|name| {
                    (
                        FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME.to_owned(),
                        name.to_string(),
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }
}

// --- Unary ---

impl<L: Clone + Loggable> LoggableBatch for L {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow([std::borrow::Cow::Borrowed(self)])
    }
}

impl<C: Component> ComponentBatch for C {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Unary Option ---

impl<L: Clone + Loggable> LoggableBatch for Option<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Option<C> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Vec ---

impl<L: Clone + Loggable> LoggableBatch for Vec<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for Vec<C> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Vec<Option> ---

impl<L: Loggable> LoggableBatch for Vec<Option<L>> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for Vec<Option<C>> {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Array ---

impl<L: Loggable, const N: usize> LoggableBatch for [L; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component, const N: usize> ComponentBatch for [C; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> LoggableBatch for [Option<L>; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component, const N: usize> ComponentBatch for [Option<C>; N] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Slice ---

impl<L: Loggable> LoggableBatch for [L] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

impl<C: Component> ComponentBatch for [C] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}

// --- Slice<Option> ---

impl<L: Loggable> LoggableBatch for [Option<L>] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

impl<C: Component> ComponentBatch for [Option<C>] {
    #[inline]
    fn descriptor(&self) -> Cow<'_, ComponentDescriptor> {
        // TODO(#6889): This is still untagged.
        ComponentDescriptor::new(C::name()).into()
    }
}
