use itertools::Itertools as _;

use crate::{Component, ComponentName, DeserializableComponent, SerializableComponent};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataCellError {
    #[error("Unsupported datatype: {0:?}")]
    UnsupportedDatatype(arrow2::datatypes::DataType),

    #[error("Could not serialize/deserialize data to/from Arrow: {0}")]
    Arrow(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type DataCellResult<T> = ::std::result::Result<T, DataCellError>;

// ---

/// A cell's worth of data, i.e. a uniform array of values for a given component type.
/// This is the leaf type in our data model.
///
/// A `DataCell` can be constructed from either an iterable of native `Component`s or directly
/// from a slice of arrow data.
///
/// Behind the scenes, a `DataCell` is backed by an erased arrow array living on the heap, which
/// is likely to point into a larger batch of contiguous memory that it shares with its peers.
/// Cloning a `DataCell` is thus cheap (shallow, ref-counted).
///
/// ## Layout
///
/// A cell is an array of component instances: `[C, C, C, ...]`.
///
/// Consider this example:
/// ```ignore
/// let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into(), [30.0, 30.0].into()];
/// let cell = DataCell::from(points);
/// // Or, alternatively:
/// let cell = DataCell::from_component::<Point2D>([[10.0, 10.0], [20.0, 20.0], [30.0, 30.0]]);
/// ```
///
/// The cell's datatype is now a `StructArray`:
/// ```ignore
/// Struct([
///    Field { name: "x", data_type: Float32, is_nullable: false, metadata: {} },
///    Field { name: "y", data_type: Float32, is_nullable: false, metadata: {} },
/// ])
/// ```
///
/// Or, visualized as a cell within a larger table:
/// ```text
/// ┌──────────────────────────────────────────────────┐
/// │ rerun.point2d                                    │
/// ╞══════════════════════════════════════════════════╡
/// │ [{x: 10, y: 10}, {x: 20, y: 20}, {x: 30, y: 30}] │
/// └──────────────────────────────────────────────────┘
/// ```
///
/// ## Example
///
/// ```rust
/// # use arrow2_convert::field::ArrowField as _;
/// # use itertools::Itertools as _;
/// #
/// # use re_log_types::{DataCell, Component as _};
/// # use re_log_types::component_types::Point2D;
/// #
/// let points: &[Point2D] = &[
///     [10.0, 10.0].into(),
///     [20.0, 20.0].into(),
///     [30.0, 30.0].into(),
/// ];
/// let _cell = DataCell::from(points);
///
/// // Or, alternatively:
/// let cell = DataCell::from_component::<Point2D>([[10.0, 10.0], [20.0, 20.0], [30.0, 30.0]]);
///
/// eprintln!("{:#?}", cell.datatype());
/// eprintln!("{cell}");
/// #
/// # assert_eq!(Point2D::name(), cell.component_name());
/// # assert_eq!(3, cell.num_instances());
/// # assert_eq!(cell.datatype(), &Point2D::data_type());
/// #
/// # assert_eq!(points, cell.as_native().collect_vec().as_slice());
/// ```
///
#[derive(Debug, Clone, PartialEq)]
pub struct DataCell {
    /// Name of the component type used in this cell.
    //
    // TODO(#1696): Store this within the datatype itself.
    pub(crate) name: ComponentName,

    /// A uniformly typed list of values for the given component type: `[C, C, C, ...]`
    ///
    /// Includes the data, its schema and probably soon the component metadata
    /// (e.g. the `ComponentName`).
    ///
    /// Internally this is always stored as an erased arrow array to avoid bad surprises with
    /// frequent boxing/unboxing down the line.
    /// Internally, this is most likely a slice of another, larger array (batching!).
    pub(crate) values: Box<dyn arrow2::array::Array>,
}

impl Default for DataCell {
    fn default() -> Self {
        Self {
            name: "".into(),
            values: arrow2::array::NullArray::new_empty(arrow2::datatypes::DataType::Null).boxed(),
        }
    }
}

// TODO(cmc): We should be able to build a cell from non-reference types.
// TODO(#1619): We shouldn't have to specify the component name separately, this should be
// part of the metadata by using an extension.
// TODO(#1696): Check that the array is indeed a leaf / component type when building a cell from an
// arrow payload.
impl DataCell {
    /// Builds a new `DataCell` from a uniform iterable of native component values.
    ///
    /// Fails if the given iterable cannot be serialized to arrow, which should never happen when
    /// using Rerun's builtin components.
    #[inline]
    pub fn try_from_native<'a, C: SerializableComponent>(
        values: impl IntoIterator<Item = &'a C>,
    ) -> DataCellResult<Self> {
        use arrow2_convert::serialize::TryIntoArrow;
        Ok(Self::from_arrow(
            C::name(),
            TryIntoArrow::try_into_arrow(values.into_iter())?,
        ))
    }

    /// Builds a new `DataCell` from a uniform iterable of native component values.
    ///
    /// Fails if the given iterable cannot be serialized to arrow, which should never happen when
    /// using Rerun's builtin components.
    #[inline]
    pub fn try_from_native_sparse<'a, C: SerializableComponent>(
        values: impl IntoIterator<Item = &'a Option<C>>,
    ) -> DataCellResult<Self> {
        use arrow2_convert::serialize::TryIntoArrow;
        Ok(Self::from_arrow(
            C::name(),
            TryIntoArrow::try_into_arrow(values.into_iter())?,
        ))
    }

    /// Builds a new `DataCell` from a uniform iterable of native component values.
    ///
    /// Panics if the given iterable cannot be serialized to arrow, which should never happen when
    /// using Rerun's builtin components.
    /// See [`Self::try_from_native`] for the fallible alternative.
    #[inline]
    pub fn from_native<'a, C: SerializableComponent>(
        values: impl IntoIterator<Item = &'a C>,
    ) -> Self {
        Self::try_from_native(values).unwrap()
    }

    /// Builds a new `DataCell` from a uniform iterable of native component values.
    ///
    /// Panics if the given iterable cannot be serialized to arrow, which should never happen when
    /// using Rerun's builtin components.
    /// See [`Self::try_from_native`] for the fallible alternative.
    #[inline]
    pub fn from_native_sparse<'a, C: SerializableComponent>(
        values: impl IntoIterator<Item = &'a Option<C>>,
    ) -> Self {
        Self::try_from_native_sparse(values).unwrap()
    }

    /// Builds a cell from an iterable of items that can be turned into a [`Component`].
    ///
    /// ⚠ Due to quirks in `arrow2-convert`, this requires consuming and collecting the passed-in
    /// iterator into a vector first.
    /// Prefer [`Self::from_native`] when performance matters.
    pub fn from_component_sparse<C>(values: impl IntoIterator<Item = Option<impl Into<C>>>) -> Self
    where
        C: SerializableComponent,
    {
        let values = values
            .into_iter()
            .map(|value| value.map(Into::into))
            .collect_vec();
        Self::from_native_sparse(values.iter())
    }

    /// Builds a cell from an iterable of items that can be turned into a [`Component`].
    ///
    /// ⚠ Due to quirks in `arrow2-convert`, this requires consuming and collecting the passed-in
    /// iterator into a vector first.
    /// Prefer [`Self::from_native`] when performance matters.
    pub fn from_component<C>(values: impl IntoIterator<Item = impl Into<C>>) -> Self
    where
        C: SerializableComponent,
    {
        let values = values.into_iter().map(Into::into).collect_vec();
        Self::from_native(values.iter())
    }

    /// Builds a new `DataCell` from an arrow array.
    ///
    /// Fails if the array is not a valid list of components.
    #[inline]
    pub fn try_from_arrow(
        name: ComponentName,
        values: Box<dyn arrow2::array::Array>,
    ) -> DataCellResult<Self> {
        Ok(Self { name, values })
    }

    /// Builds a new `DataCell` from an arrow array.
    ///
    /// Panics if the array is not a valid list of components.
    /// See [`Self::try_from_arrow`] for the fallible alternative.
    #[inline]
    pub fn from_arrow(name: ComponentName, values: Box<dyn arrow2::array::Array>) -> Self {
        Self::try_from_arrow(name, values).unwrap()
    }

    // ---

    /// Builds an empty `DataCell` from a native component type.
    //
    // TODO(#1595): do keep in mind there's a future not too far away where components become a
    // `(component, type)` tuple kinda thing.
    #[inline]
    pub fn from_native_empty<C: Component>() -> Self {
        Self::from_arrow_empty(C::name(), C::data_type())
    }

    /// Builds an empty `DataCell` from an arrow datatype.
    ///
    /// Fails if the datatype is not a valid component type.
    #[inline]
    pub fn try_from_arrow_empty(
        name: ComponentName,
        datatype: arrow2::datatypes::DataType,
    ) -> DataCellResult<Self> {
        // TODO(cmc): check that it is indeed a component datatype
        Ok(Self {
            name,
            values: arrow2::array::new_empty_array(datatype),
        })
    }

    /// Builds an empty `DataCell` from an arrow datatype.
    ///
    /// Panics if the datatype is not a valid component type.
    /// See [`Self::try_from_arrow_empty`] for a fallible alternative.
    #[inline]
    pub fn from_arrow_empty(name: ComponentName, datatype: arrow2::datatypes::DataType) -> Self {
        Self::try_from_arrow_empty(name, datatype).unwrap()
    }

    // ---

    /// Returns the contents of the cell as an arrow array.
    ///
    /// Avoid using raw arrow arrays unless you absolutely have to: prefer working directly with
    /// `DataCell`s, `DataRow`s & `DataTable`s instead.
    /// If you do use them, try to keep the scope as short as possible: holding on to a raw array
    /// might prevent the datastore from releasing memory from garbage collected data.
    #[inline]
    pub fn into_arrow(self) -> Box<dyn arrow2::array::Array> {
        self.values
    }

    /// Returns the contents of the cell as an arrow array (shallow clone).
    ///
    /// Avoid using raw arrow arrays unless you absolutely have to: prefer working directly with
    /// `DataCell`s, `DataRow`s & `DataTable`s instead.
    /// If you do use them, try to keep the scope as short as possible: holding on to a raw array
    /// might prevent the datastore from releasing memory from garbage collected data.
    #[inline]
    pub fn as_arrow(&self) -> Box<dyn arrow2::array::Array> {
        self.values.clone() /* shallow */
    }

    /// Returns the contents of the cell as a reference to an arrow array.
    ///
    /// Avoid using raw arrow arrays unless you absolutely have to: prefer working directly with
    /// `DataCell`s, `DataRow`s & `DataTable`s instead.
    /// If you do use them, try to keep the scope as short as possible: holding on to a raw array
    /// might prevent the datastore from releasing memory from garbage collected data.
    #[inline]
    pub fn as_arrow_ref(&self) -> &dyn arrow2::array::Array {
        &*self.values
    }

    /// Returns the contents of the cell as an arrow array (shallow clone) wrapped in a unit-length
    /// list-array.
    ///
    /// Useful when dealing with cells of different lengths in context that don't allow for it.
    ///
    /// * Before: `[C, C, C, ...]`
    /// * After: `ListArray[ [C, C, C, C] ]`
    //
    // TODO(#1696): this shouldn't be public, need to make it private once the store has been
    // patched to use datacells directly.
    // TODO(cmc): effectively, this returns a `DataColumn`... think about that.
    #[doc(hidden)]
    #[inline]
    pub fn as_arrow_monolist(&self) -> Box<dyn arrow2::array::Array> {
        use arrow2::{array::ListArray, offset::Offsets};

        let values = self.as_arrow();
        let datatype = self.datatype().clone();

        let datatype = ListArray::<i32>::default_datatype(datatype);
        let offsets = Offsets::try_from_lengths(std::iter::once(self.num_instances() as usize))
            .unwrap()
            .into();
        let validity = None;

        ListArray::<i32>::new(datatype, offsets, values, validity).boxed()
    }

    /// Returns the contents of the cell as an iterator of native components.
    ///
    /// Fails if the underlying arrow data cannot be deserialized into `C`.
    //
    // TODO(#1694): There shouldn't need to be HRTBs (Higher-Rank Trait Bounds) here.
    #[inline]
    pub fn try_as_native<C: DeserializableComponent>(
        &self,
    ) -> DataCellResult<impl Iterator<Item = C> + '_>
    where
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        use arrow2_convert::deserialize::arrow_array_deserialize_iterator;
        arrow_array_deserialize_iterator(&*self.values).map_err(Into::into)
    }

    /// Returns the contents of the cell as an iterator of native components.
    ///
    /// Panics if the underlying arrow data cannot be deserialized into `C`.
    /// See [`Self::try_as_native`] for a fallible alternative.
    //
    // TODO(#1694): There shouldn't need to be HRTBs here.
    #[inline]
    pub fn as_native<C: DeserializableComponent>(&self) -> impl Iterator<Item = C> + '_
    where
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        self.try_as_native().unwrap()
    }
}

impl DataCell {
    /// The name of the component type stored in the cell.
    #[inline]
    pub fn component_name(&self) -> ComponentName {
        self.name
    }

    /// The type of the component stored in the cell, i.e. the cell is an array of that type.
    #[inline]
    pub fn datatype(&self) -> &arrow2::datatypes::DataType {
        self.values.data_type()
    }

    /// The length of the cell's array, i.e. how many component instances are in the cell?
    #[inline]
    pub fn num_instances(&self) -> u32 {
        self.values.len() as _
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns `true` if the underlying array is dense (no nulls).
    #[inline]
    pub fn is_dense(&self) -> bool {
        if let Some(validity) = self.as_arrow_ref().validity() {
            validity.unset_bits() == 0
        } else {
            true
        }
    }

    /// Returns `true` if the underlying array is both sorted (increasing order) and contains only
    /// unique values.
    ///
    /// The cell must be dense, otherwise the result of this method is undefined.
    pub fn is_sorted_and_unique(&self) -> DataCellResult<bool> {
        use arrow2::{
            array::{Array, PrimitiveArray},
            datatypes::DataType,
            types::NativeType,
        };

        debug_assert!(self.is_dense());

        let arr = self.as_arrow_ref();

        fn is_sorted_and_unique_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            // NOTE: unwrap cannot fail, checked by caller just below
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            values.values().windows(2).all(|v| v[0] < v[1])
        }

        // TODO(cmc): support more datatypes as the need arise.
        match arr.data_type() {
            DataType::Int8 => Ok(is_sorted_and_unique_primitive::<i8>(arr)),
            DataType::Int16 => Ok(is_sorted_and_unique_primitive::<i16>(arr)),
            DataType::Int32 => Ok(is_sorted_and_unique_primitive::<i32>(arr)),
            DataType::Int64 => Ok(is_sorted_and_unique_primitive::<i64>(arr)),
            DataType::UInt8 => Ok(is_sorted_and_unique_primitive::<u8>(arr)),
            DataType::UInt16 => Ok(is_sorted_and_unique_primitive::<u16>(arr)),
            DataType::UInt32 => Ok(is_sorted_and_unique_primitive::<u32>(arr)),
            DataType::UInt64 => Ok(is_sorted_and_unique_primitive::<u64>(arr)),
            DataType::Float32 => Ok(is_sorted_and_unique_primitive::<f32>(arr)),
            DataType::Float64 => Ok(is_sorted_and_unique_primitive::<f64>(arr)),
            _ => Err(DataCellError::UnsupportedDatatype(arr.data_type().clone())),
        }
    }
}

// ---

// TODO(#1693): this should be `C: Component`, nothing else.

impl<C: SerializableComponent> From<&[C]> for DataCell {
    #[inline]
    fn from(values: &[C]) -> Self {
        Self::from_native(values.iter())
    }
}

impl<C: SerializableComponent> From<Vec<C>> for DataCell {
    #[inline]
    fn from(c: Vec<C>) -> Self {
        c.as_slice().into()
    }
}

impl<C: SerializableComponent> From<&Vec<C>> for DataCell {
    #[inline]
    fn from(c: &Vec<C>) -> Self {
        c.as_slice().into()
    }
}

// ---

impl std::fmt::Display for DataCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format::arrow::format_table(
            // NOTE: wrap in a ListArray so that it looks more cell-like (i.e. single row)
            [&*self.as_arrow_monolist()],
            [self.component_name()],
        )
        .fmt(f)
    }
}

// ---

impl DataCell {
    /// Returns the total (heap) allocated size of the array in bytes.
    ///
    /// Beware: this is costly! Cache the returned value as much as possible.
    pub fn size_bytes(&self) -> u64 {
        let Self { name, values } = self;

        std::mem::size_of_val(name) as u64 +
            // Warning: this is surprisingly costly!
            arrow2::compute::aggregate::estimated_bytes_size(&**values) as u64
    }
}

// This test exists because the documentation and online discussions revolving around
// arrow2's `estimated_bytes_size()` function indicate that there's a lot of limitations and
// edge cases to be aware of.
//
// Also, it's just plain hard to be sure that the answer you get is the answer you're looking
// for with these kinds of tools. When in doubt.. test everything we're going to need from it.
//
// In many ways, this is a specification of what we mean when we ask "what's the size of this
// Arrow array?".
#[test]
#[allow(clippy::from_iter_instead_of_collect)]
fn test_arrow_estimated_size_bytes() {
    use arrow2::{
        array::{Array, Float64Array, ListArray, StructArray, UInt64Array, Utf8Array},
        compute::aggregate::estimated_bytes_size,
        datatypes::{DataType, Field},
        offset::Offsets,
    };

    // simple primitive array
    {
        let data = vec![42u64; 100];
        let array = UInt64Array::from_vec(data.clone()).boxed();
        assert_eq!(
            std::mem::size_of_val(data.as_slice()),
            estimated_bytes_size(&*array)
        );
    }

    // utf8 strings array
    {
        let data = vec![Some("some very, very, very long string indeed"); 100];
        let array = Utf8Array::<i32>::from(data.clone()).to_boxed();

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.unwrap().as_bytes()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(5600, raw_size_bytes);
        assert_eq!(4404, arrow_size_bytes); // smaller because validity bitmaps instead of opts
    }

    // simple primitive list array
    {
        let data = std::iter::repeat(vec![42u64; 100])
            .take(50)
            .collect::<Vec<_>>();
        let array = {
            let array_flattened =
                UInt64Array::from_vec(data.clone().into_iter().flatten().collect()).boxed();

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(DataType::UInt64),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array_flattened,
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(41200, raw_size_bytes);
        assert_eq!(40200, arrow_size_bytes); // smaller because smaller inner headers
    }

    // compound type array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }

        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = vec![Point::default(); 100];
        let array = {
            let x = Float64Array::from_vec(data.iter().map(|p| p.x).collect()).boxed();
            let y = Float64Array::from_vec(data.iter().map(|p| p.y).collect()).boxed();
            let fields = vec![
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
            ];
            StructArray::new(DataType::Struct(fields), vec![x, y], None).boxed()
        };

        let raw_size_bytes = std::mem::size_of_val(data.as_slice());
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(1600, raw_size_bytes);
        assert_eq!(1600, arrow_size_bytes);
    }

    // compound type list array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }

        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = std::iter::repeat(vec![Point::default(); 100])
            .take(50)
            .collect::<Vec<_>>();
        let array: Box<dyn Array> = {
            let array = {
                let x =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.x).collect()).boxed();
                let y =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.y).collect()).boxed();
                let fields = vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                ];
                StructArray::new(DataType::Struct(fields), vec![x, y], None)
            };

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(array.data_type().clone()),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array.boxed(),
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(81200, raw_size_bytes);
        assert_eq!(80200, arrow_size_bytes); // smaller because smaller inner headers
    }
}
