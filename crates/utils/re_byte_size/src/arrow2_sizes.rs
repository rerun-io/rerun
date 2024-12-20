//! --- Arrow2 size estimations ---
//!
//! The following is a modified version of <https://github.com/jorgecarleitao/arrow2/blob/v0.16.0/src/compute/aggregate/memory.rs>,
//! available under MIT OR Apache-2.0.

use std::collections::BTreeMap;

use arrow2::{
    array::{
        Array, BinaryArray, BooleanArray, DictionaryArray, FixedSizeBinaryArray,
        FixedSizeListArray, ListArray, MapArray, PrimitiveArray, StructArray, UnionArray,
        Utf8Array,
    },
    bitmap::Bitmap,
    datatypes::{DataType, Field, PhysicalType},
    types::{NativeType, Offset},
};

use super::SizeBytes;

macro_rules! with_match_primitive_type {(
    $key_type:expr, | $_:tt $T:ident | $($body:tt)*
) => ({
    macro_rules! __with_ty__ {( $_ $T:ident ) => ( $($body)* )}
    use arrow2::datatypes::PrimitiveType::*;
    use arrow2::types::{days_ms, months_days_ns, f16, i256};
    match $key_type {
        Int8 => __with_ty__! { i8 },
        Int16 => __with_ty__! { i16 },
        Int32 => __with_ty__! { i32 },
        Int64 => __with_ty__! { i64 },
        Int128 => __with_ty__! { i128 },
        Int256 => __with_ty__! { i256 },
        DaysMs => __with_ty__! { days_ms },
        MonthDayNano => __with_ty__! { months_days_ns },
        UInt8 => __with_ty__! { u8 },
        UInt16 => __with_ty__! { u16 },
        UInt32 => __with_ty__! { u32 },
        UInt64 => __with_ty__! { u64 },
        Float16 => __with_ty__! { f16 },
        Float32 => __with_ty__! { f32 },
        Float64 => __with_ty__! { f64 },
    }
})}

macro_rules! match_integer_type {(
    $key_type:expr, | $_:tt $T:ident | $($body:tt)*
) => ({
    macro_rules! __with_ty__ {( $_ $T:ident ) => ( $($body)* )}
    use arrow2::datatypes::IntegerType::*;
    match $key_type {
        Int8 => __with_ty__! { i8 },
        Int16 => __with_ty__! { i16 },
        Int32 => __with_ty__! { i32 },
        Int64 => __with_ty__! { i64 },
        UInt8 => __with_ty__! { u8 },
        UInt16 => __with_ty__! { u16 },
        UInt32 => __with_ty__! { u32 },
        UInt64 => __with_ty__! { u64 },
    }
})}

macro_rules! dyn_binary {
    ($array:expr, $ty:ty, $o:ty) => {{
        let array = $array.as_any().downcast_ref::<$ty>().unwrap();
        let offsets = array.offsets().buffer();

        // in case of Binary/Utf8/List the offsets are sliced,
        // not the values buffer
        let values_start = offsets[0] as usize;
        let values_end = offsets[offsets.len() - 1] as usize;

        values_end - values_start
            + offsets.len() * std::mem::size_of::<$o>()
            + validity_size(array.validity())
    }};
}

fn validity_size(validity: Option<&Bitmap>) -> usize {
    validity.as_ref().map_or(0, |b| b.as_slice().0.len())
}

/// Returns the total (heap) allocated size of the array in bytes.
///
/// # Implementation
/// This estimation is the sum of the size of its buffers, validity, including nested arrays.
/// Multiple arrays may share buffers and bitmaps. Therefore, the size of 2 arrays is not the
/// sum of the sizes computed from this function. In particular, [`StructArray`]'s size is an upper bound.
///
/// When an array is sliced, its allocated size remains constant because the buffer unchanged.
/// However, this function will yield a smaller number. This is because this function returns
/// the visible size of the buffer, not its total capacity.
///
/// FFI buffers are included in this estimation.
fn estimated_bytes_size(array: &dyn Array) -> usize {
    #![allow(clippy::unwrap_used)] // We check the type first

    // NOTE: `.len()` is the number of elements in an arrow2 buffer
    // no matter WHAT the documentation says.
    // See https://github.com/jorgecarleitao/arrow2/issues/1430

    #[allow(clippy::enum_glob_use)]
    use PhysicalType::*;

    match array.data_type().to_physical_type() {
        Null => 0,
        Boolean => {
            let array = array.as_any().downcast_ref::<BooleanArray>().unwrap();
            array.values().as_slice().0.len() + validity_size(array.validity())
        }
        Primitive(primitive) => with_match_primitive_type!(primitive, |$T| {
            let array = array
                .as_any()
                .downcast_ref::<PrimitiveArray<$T>>()
                .unwrap();
            array.values().len() * std::mem::size_of::<$T>() + validity_size(array.validity())
        }),
        Binary => dyn_binary!(array, BinaryArray<i32>, i32),
        FixedSizeBinary => {
            let array = array
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            array.values().len() + validity_size(array.validity())
        }
        LargeBinary => dyn_binary!(array, BinaryArray<i64>, i64),
        Utf8 => dyn_binary!(array, Utf8Array<i32>, i32),
        LargeUtf8 => dyn_binary!(array, Utf8Array<i64>, i64),
        // NOTE: Diverges from upstream.
        List | LargeList => {
            let array = array.as_any().downcast_ref::<ListArray<i32>>().unwrap();

            let offsets = array.offsets().buffer();
            let values_start = offsets[0] as usize;
            let values_end = offsets[offsets.len() - 1] as usize;

            estimated_bytes_size(
                array
                    .values()
                    .sliced(values_start, values_end - values_start)
                    .as_ref(),
            ) + std::mem::size_of_val(array.offsets().as_slice())
                + validity_size(array.validity())
        }
        FixedSizeList => {
            let array = array.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
            estimated_bytes_size(array.values().as_ref()) + validity_size(array.validity())
        }
        Struct => {
            let array = array.as_any().downcast_ref::<StructArray>().unwrap();
            array
                .values()
                .iter()
                .map(|x| x.as_ref())
                .map(estimated_bytes_size)
                .sum::<usize>()
                + validity_size(array.validity())
        }
        // NOTE: Diverges from upstream.
        Union => {
            let array = array.as_any().downcast_ref::<UnionArray>().unwrap();

            let types_size = array.types().len() * std::mem::size_of::<i8>();

            if let Some(offsets) = array.offsets() {
                // https://arrow.apache.org/docs/format/Columnar.html#dense-union:
                //
                // Dense union represents a mixed-type array with 5 bytes of overhead for each
                // value. Its physical layout is as follows:
                // - One child array for each type.
                // - Types buffer: A buffer of 8-bit signed integers. Each type in the union has a
                //   corresponding type id whose values are found in this buffer.
                //   A union with more than 127 possible types can be modeled as a union of unions.
                // - Offsets buffer: A buffer of signed Int32 values indicating the relative
                //   offset into the respective child array for the type in a given slot.
                //   The respective offsets for each child value array must be in
                //   order / increasing.

                /// The range of offsets for a given type id.
                #[derive(Debug)]
                struct Range {
                    /// Inclusive
                    min: i32,

                    /// Inclusive
                    max: i32,
                }

                // The range of offsets for a given type id.
                let mut type_ranges: BTreeMap<i8, Range> = Default::default();

                debug_assert_eq!(array.types().len(), offsets.len());
                for (&type_id, &offset) in array.types().iter().zip(offsets.iter()) {
                    // Offsets are monotonically increasing
                    type_ranges
                        .entry(type_id)
                        .and_modify(|range| {
                            range.max = offset;
                        })
                        .or_insert(Range {
                            min: offset,
                            max: offset,
                        });
                }

                let mut fields_size = 0;
                for (type_id, range) in type_ranges {
                    if let Some(field) = array.fields().get(type_id as usize) {
                        let len = range.max - range.min + 1; // range is inclusive
                        fields_size += estimated_bytes_size(
                            field.sliced(range.min as usize, len as usize).as_ref(),
                        );
                    }
                }

                let offsets_size = offsets.len() * std::mem::size_of::<i32>();

                types_size + offsets_size + fields_size
            } else {
                // https://arrow.apache.org/docs/format/Columnar.html#sparse-union:
                //
                // A sparse union has the same structure as a dense union, with the omission of
                // the offsets array. In this case, the child arrays are each equal in length to
                // the length of the union.
                //
                // While a sparse union may use significantly more space compared with a dense
                // union, it has some advantages that may be desirable in certain use cases:
                // - A sparse union is more amenable to vectorized expression evaluation in some
                //   use cases.
                // - Equal-length arrays can be interpreted as a union by only defining the types
                //   array.

                // Rerun uses sparse unions to encode C-style enums.
                // In that case, each field type is `null`, and the arrays are empty.

                let num_elems = array.types().len();
                let fields_size = array
                    .fields()
                    .iter()
                    .map(|x| estimated_bytes_size(x.sliced(0, num_elems.min(x.len())).as_ref()))
                    .sum::<usize>();
                types_size + fields_size
            }
        }
        Dictionary(key_type) => match_integer_type!(key_type, |$T| {
            let array = array
                .as_any()
                .downcast_ref::<DictionaryArray<$T>>()
                .unwrap();
            estimated_bytes_size(array.keys()) + estimated_bytes_size(array.values().as_ref())
        }),
        Map => {
            let array = array.as_any().downcast_ref::<MapArray>().unwrap();
            let offsets = array.offsets().len() * std::mem::size_of::<i32>();
            offsets + estimated_bytes_size(array.field().as_ref()) + validity_size(array.validity())
        }
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
fn test_arrow2_estimated_size_bytes() {
    use arrow2::{
        array::{Array, Float64Array, ListArray, StructArray, UInt64Array, Utf8Array},
        buffer::Buffer,
        datatypes::{DataType, Field, UnionMode},
        offset::Offsets,
    };
    use std::mem::size_of;
    use std::sync::Arc;

    // empty primitive array
    {
        let data = vec![];
        let array = UInt64Array::from_vec(data.clone()).boxed();
        let sz = estimated_bytes_size(&*array);
        assert_eq!(0, sz);
        assert_eq!(std::mem::size_of_val(data.as_slice()), sz);
    }

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
                Offsets::try_from_lengths(std::iter::repeat(100).take(50))
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
        assert_eq!(40204, arrow_size_bytes); // smaller because smaller inner headers
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
            StructArray::new(DataType::Struct(Arc::new(fields)), vec![x, y], None).boxed()
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
                StructArray::new(DataType::Struct(Arc::new(fields)), vec![x, y], None)
            };

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(array.data_type().clone()),
                Offsets::try_from_lengths(std::iter::repeat(100).take(50))
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
        assert_eq!(80204, arrow_size_bytes); // smaller because smaller inner headers
    }

    // Dense union, `enum { i(i32), f(f32) }`
    {
        let fields = vec![
            Field::new("i", DataType::Int32, false),
            Field::new("f", DataType::Float64, false),
        ];
        let data_type = DataType::Union(
            Arc::new(fields),
            Some(Arc::new(vec![0i32, 1i32])),
            UnionMode::Dense,
        );
        let types = Buffer::<i8>::from(vec![0i8, 0i8, 1i8, 0i8, 1i8]);
        let fields = vec![
            PrimitiveArray::<i32>::from_vec(vec![0, 1, 2]).boxed(),
            PrimitiveArray::<f64>::from_vec(vec![0.0, 1.0]).boxed(),
        ];
        let offsets = vec![0, 1, 0, 2, 1];
        let array = UnionArray::new(data_type, types, fields, Some(offsets.into())).boxed();

        let raw_size_bytes = 5 + 3 * size_of::<i32>() + 2 * size_of::<f64>() + 5 * size_of::<i32>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(raw_size_bytes, arrow_size_bytes);
    }
}

impl SizeBytes for DataType {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Null
            | Self::Binary
            | Self::Boolean
            | Self::Date32
            | Self::Date64
            | Self::Float16
            | Self::Float32
            | Self::Float64
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::Int8
            | Self::LargeBinary
            | Self::LargeUtf8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::UInt8
            | Self::Time32(_)
            | Self::Time64(_)
            | Self::Duration(_)
            | Self::Interval(_)
            | Self::FixedSizeBinary(_)
            | Self::Decimal(_, _)
            | Self::Decimal256(_, _)
            | Self::Utf8 => 0,
            Self::Timestamp(_, str) => str.heap_size_bytes(),
            Self::List(field)
            | Self::FixedSizeList(field, _)
            | Self::LargeList(field)
            | Self::Map(field, _) => field.total_size_bytes(), // NOTE: Boxed, it's all on the heap
            Self::Struct(fields) => fields.heap_size_bytes(),
            Self::Union(fields, indices, _) => fields.heap_size_bytes() + indices.heap_size_bytes(),
            Self::Dictionary(_, datatype, _) => datatype.total_size_bytes(), // NOTE: Boxed, it's all on the heap
            Self::Extension(name, datatype, extra) => {
                name.heap_size_bytes()
                + datatype.total_size_bytes() // NOTE: Boxed, it's all on the heap
                + extra.heap_size_bytes()
            }
        }
    }
}

impl SizeBytes for Field {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            name,
            data_type,
            is_nullable,
            metadata,
        } = self;

        name.heap_size_bytes()
            + data_type.heap_size_bytes()
            + is_nullable.heap_size_bytes()
            + metadata.heap_size_bytes()
    }
}

impl SizeBytes for dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        estimated_bytes_size(self) as _
    }
}

impl SizeBytes for Box<dyn Array> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        estimated_bytes_size(&**self as _) as _
    }
}

impl<T: SizeBytes + NativeType> SizeBytes for PrimitiveArray<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        estimated_bytes_size(self) as _
    }
}

impl<T: SizeBytes + Offset> SizeBytes for ListArray<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        estimated_bytes_size(self) as _
    }
}

impl SizeBytes for StructArray {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        estimated_bytes_size(self) as _
    }
}
