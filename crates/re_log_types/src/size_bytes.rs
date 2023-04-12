use std::collections::{BTreeMap, HashMap};

use arrow2::datatypes::{DataType, Field};
use smallvec::SmallVec;

// ---

/// Approximations of stack and heap size for both internal and external types.
///
/// Motly used for statistics and triggering events such as garbage collection.
pub trait SizeBytes: Sized {
    /// Returns the total size of `self` in bytes, accounting for both stack and heap space.
    #[inline]
    fn total_size_bytes(&self) -> u64 {
        self.stack_size_bytes() + self.heap_size_bytes()
    }

    /// Returns the total size of `self` on the stack, in bytes.
    ///
    /// Defaults to `std::mem::size_of_val(self)`.
    #[inline]
    fn stack_size_bytes(&self) -> u64 {
        std::mem::size_of_val(self) as _
    }

    /// Returns the total size of `self` on the heap, in bytes.
    fn heap_size_bytes(&self) -> u64;
}

// --- Std ---

impl SizeBytes for String {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.capacity() as u64
    }
}

impl<K: SizeBytes, V: SizeBytes> SizeBytes for BTreeMap<K, V> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): This is sub-optimal if these types are PODs.

        // NOTE: It's all on the heap at this point.
        self.keys().map(SizeBytes::total_size_bytes).sum::<u64>()
            + self.values().map(SizeBytes::total_size_bytes).sum::<u64>()
    }
}

impl<K: SizeBytes, V: SizeBytes, S> SizeBytes for HashMap<K, V, S> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): This is sub-optimal if these types are PODs.

        // NOTE: It's all on the heap at this point.
        self.keys().map(SizeBytes::total_size_bytes).sum::<u64>()
            + self.values().map(SizeBytes::total_size_bytes).sum::<u64>()
    }
}

impl<T: SizeBytes> SizeBytes for Vec<T> {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): This is sub-optimal if these types are PODs.

        // NOTE: It's all on the heap at this point.
        self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
    }
}

impl<T: SizeBytes, const N: usize> SizeBytes for SmallVec<[T; N]> {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): This is sub-optimal if these types are PODs.

        // NOTE: It's all on the heap at this point.
        self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
    }
}

impl<T: SizeBytes> SizeBytes for Option<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.as_ref().map_or(0, SizeBytes::heap_size_bytes)
    }
}

// NOTE: `impl<T: bytemuck::Pod> SizeBytesExt for T {}` would be nice but violates orphan rules.
macro_rules! impl_size_bytes_pod {
    ($ty:ty) => {
        impl SizeBytes for $ty {
            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }
        }
    };
    ($ty:ty, $($rest:ty),+) => {
        impl_size_bytes_pod!($ty); impl_size_bytes_pod!($($rest),+);
    };
}

impl_size_bytes_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, f32, f64);

// --- Arrow ---

impl SizeBytes for DataType {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            DataType::Null
            | DataType::Binary
            | DataType::Boolean
            | DataType::Date32
            | DataType::Date64
            | DataType::Float16
            | DataType::Float32
            | DataType::Float64
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Int8
            | DataType::LargeBinary
            | DataType::LargeUtf8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::UInt8
            | DataType::Time32(_)
            | DataType::Time64(_)
            | DataType::Duration(_)
            | DataType::Interval(_)
            | DataType::FixedSizeBinary(_)
            | DataType::Decimal(_, _)
            | DataType::Decimal256(_, _)
            | DataType::Utf8 => 0,
            DataType::Timestamp(_, str) => str.heap_size_bytes(),
            DataType::List(field)
            | DataType::FixedSizeList(field, _)
            | DataType::LargeList(field)
            | DataType::Map(field, _) => field.total_size_bytes(), // NOTE: Boxed, it's all on the heap
            DataType::Struct(fields) => fields.heap_size_bytes(),
            DataType::Union(fields, indices, _) => {
                fields.heap_size_bytes() + indices.heap_size_bytes()
            }
            DataType::Dictionary(_, datatype, _) => datatype.total_size_bytes(), // NOTE: Boxed, it's all on the heap
            DataType::Extension(name, datatype, extra) => {
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
        let Field {
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
