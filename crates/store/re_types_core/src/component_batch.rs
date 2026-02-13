use arrow::array::{ListArray as ArrowListArray, ListArray};
use arrow::buffer::OffsetBuffer;

// used in docstrings:
#[allow(clippy::allow_attributes, unused_imports, clippy::unused_trait_names)]
use crate::Archetype;
use crate::{ArchetypeName, ComponentDescriptor, ComponentType, Loggable, SerializationResult};

// ---

/// A [`ComponentBatch`] represents an array's worth of [`Loggable`] instances, ready to be
/// serialized.
///
/// [`ComponentBatch`] is carefully designed to be erasable ("object-safe"), so that it is possible
/// to build heterogeneous collections of [`ComponentBatch`]s (e.g. `Vec<dyn ComponentBatch>`).
/// This erasability is what makes extending [`Archetype`]s possible with little effort.
///
/// You should almost never need to implement [`ComponentBatch`] manually, as it is already
/// blanket implemented for most common use cases (arrays/vectors/slices of loggables, etc).
pub trait ComponentBatch {
    // NOTE: It'd be tempting to have the following associated type, but that'd be
    // counterproductive, the whole point of this is to allow for heterogeneous collections!
    // type Loggable: Loggable;

    /// Serializes the batch into an Arrow array.
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef>;

    /// Serializes the batch into an Arrow list array with a single component per list.
    fn to_arrow_list_array(&self) -> SerializationResult<ArrowListArray> {
        let array = self.to_arrow()?;
        let offsets =
            arrow::buffer::OffsetBuffer::from_lengths(std::iter::repeat_n(1, array.len()));
        let nullable = true;
        let field = arrow::datatypes::Field::new("item", array.data_type().clone(), nullable);
        ArrowListArray::try_new(field.into(), offsets, array, None).map_err(|err| err.into())
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
}

#[expect(dead_code)]
fn assert_component_batch_object_safe() {
    let _: &dyn ComponentBatch;
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
    // TODO(cmc): Maybe Cow<> this one if it grows bigger. Or intern descriptors altogether, most likely.
    pub descriptor: ComponentDescriptor,

    pub array: arrow::array::ArrayRef,
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
        Self { descriptor, array }
    }

    #[inline]
    pub fn with_descriptor_override(self, descriptor: ComponentDescriptor) -> Self {
        Self { descriptor, ..self }
    }

    /// Unconditionally sets the descriptor's `archetype_name` to the given one.
    #[inline]
    pub fn with_archetype(mut self, archetype_name: ArchetypeName) -> Self {
        self.descriptor = self.descriptor.with_archetype(archetype_name);
        self
    }

    /// Unconditionally sets the descriptor's `component_type` to the given one.
    #[inline]
    pub fn with_component_type(mut self, component_type: ComponentType) -> Self {
        self.descriptor = self.descriptor.with_component_type(component_type);
        self
    }

    /// Sets the descriptor's `archetype_name` to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype(mut self, archetype_name: impl Fn() -> ArchetypeName) -> Self {
        self.descriptor = self.descriptor.or_with_archetype(archetype_name);
        self
    }

    /// Sets the descriptor's `component` to the given one iff it's not already set.
    #[inline]
    pub fn or_with_component_type(
        mut self,
        component_type: impl FnOnce() -> ComponentType,
    ) -> Self {
        self.descriptor = self.descriptor.or_with_component_type(component_type);
        self
    }
}

/// A column's worth of component data.
///
/// If a [`SerializedComponentBatch`] represents one row's worth of data
#[derive(Debug, Clone, PartialEq)]
pub struct SerializedComponentColumn {
    pub list_array: arrow::array::ListArray,

    // TODO(cmc): Maybe Cow<> this one if it grows bigger. Or intern descriptors altogether, most likely.
    pub descriptor: ComponentDescriptor,
}

impl SerializedComponentColumn {
    #[inline]
    pub fn new(list_array: arrow::array::ListArray, descriptor: ComponentDescriptor) -> Self {
        Self {
            list_array,
            descriptor,
        }
    }

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

        let list_array = repartition_list_array(list_array, lengths)?;

        Ok(Self {
            list_array,
            descriptor,
        })
    }
}

impl re_byte_size::SizeBytes for SerializedComponentColumn {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.list_array.heap_size_bytes() + self.descriptor.heap_size_bytes()
    }
}

impl From<SerializedComponentBatch> for SerializedComponentColumn {
    #[inline]
    fn from(batch: SerializedComponentBatch) -> Self {
        use arrow::array::{Array as _, ListArray};
        use arrow::buffer::OffsetBuffer;
        use arrow::datatypes::Field;

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

/// Repartitions a [`ListArray`] according to the specified `lengths`, ignoring previous partitioning.
///
/// The specified `lengths` must sum to the total length underlying values (i.e. the child array).
///
/// The validity of the values is ignored.
#[inline]
pub fn repartition_list_array(
    list_array: ListArray,
    lengths: impl IntoIterator<Item = usize>,
) -> arrow::error::Result<ListArray> {
    let (field, _offsets, values, _nulls) = list_array.into_parts();

    let offsets = OffsetBuffer::from_lengths(lengths);
    let nulls = None;

    ListArray::try_new(field, offsets, values, nulls)
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

    /// Partitions the component data into a single-column batch with one item per row.
    ///
    /// See also [`SerializedComponentBatch::partitioned`].
    #[inline]
    pub fn column_of_unit_batches(self) -> SerializationResult<SerializedComponentColumn> {
        let len = self.array.len();
        self.partitioned(std::iter::repeat_n(1, len))
    }
}

// ---

// TODO(cmc): This is far from ideal and feels very hackish, but for now the priority is getting
// all things related to tags up and running so we can gather learnings.
// This is only used on the archetype deserialization path, which isn't ever used outside of tests anyway.

impl From<&SerializedComponentBatch> for arrow::datatypes::Field {
    #[inline]
    fn from(batch: &SerializedComponentBatch) -> Self {
        Self::new(
            batch.descriptor.component.to_string(),
            batch.array.data_type().clone(),
            false,
        )
        .with_metadata(
            [
                batch.descriptor.archetype.map(|name| {
                    (
                        crate::FIELD_METADATA_KEY_ARCHETYPE.to_owned(),
                        name.to_string(),
                    )
                }),
                Some((
                    crate::FIELD_METADATA_KEY_COMPONENT.to_owned(),
                    batch.descriptor.component.to_string(),
                )),
                batch.descriptor.component_type.map(|name| {
                    (
                        crate::FIELD_METADATA_KEY_COMPONENT_TYPE.to_owned(),
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

impl<L: Clone + Loggable> ComponentBatch for L {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow([std::borrow::Cow::Borrowed(self)])
    }
}

// --- Unary Option ---

impl<L: Clone + Loggable> ComponentBatch for Option<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

// --- Vec ---

impl<L: Clone + Loggable> ComponentBatch for Vec<L> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

// --- Vec<Option> ---

impl<L: Loggable> ComponentBatch for Vec<Option<L>> {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

// --- Array ---

impl<L: Loggable, const N: usize> ComponentBatch for [L; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

// --- Array<Option> ---

impl<L: Loggable, const N: usize> ComponentBatch for [Option<L>; N] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}

// --- Slice ---

impl<L: Loggable> ComponentBatch for [L] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow(self.iter().map(|v| std::borrow::Cow::Borrowed(v)))
    }
}

// --- Slice<Option> ---

impl<L: Loggable> ComponentBatch for [Option<L>] {
    #[inline]
    fn to_arrow(&self) -> SerializationResult<arrow::array::ArrayRef> {
        L::to_arrow_opt(
            self.iter()
                .map(|opt| opt.as_ref().map(|v| std::borrow::Cow::Borrowed(v))),
        )
    }
}
