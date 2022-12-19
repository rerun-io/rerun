use anyhow::bail;
use arrow2::{
    array::{Array, PrimitiveArray},
    datatypes::DataType,
    types::NativeType,
};

// ---

pub trait ArrayExt: Array {
    /// Returns `true` if the array is dense (no nulls).
    fn is_dense(&self) -> bool;

    /// Returns `true` if the array is sorted (increasing order).
    ///
    /// The array must be dense, otherwise the result of this method is undefined.
    fn is_sorted(&self) -> anyhow::Result<bool>;

    /// Returns `true` if the array contains any duplicated values.
    ///
    /// The array must be sorted, otherwise the result of this method is undefined.
    fn contains_duplicates(&self) -> anyhow::Result<bool>;
}

impl ArrayExt for dyn Array {
    fn is_dense(&self) -> bool {
        self.validity().is_none()
    }

    fn is_sorted(&self) -> anyhow::Result<bool> {
        debug_assert!(self.is_dense());

        fn is_sorted_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            values.values().windows(2).all(|v| v[0] <= v[1])
        }

        // TODO(cmc): support more datatypes as the need arise.
        match self.data_type() {
            DataType::Int8 => Ok(is_sorted_primitive::<i8>(self)),
            DataType::Int16 => Ok(is_sorted_primitive::<i16>(self)),
            DataType::Int32 => Ok(is_sorted_primitive::<i32>(self)),
            DataType::Int64 => Ok(is_sorted_primitive::<i64>(self)),
            DataType::UInt8 => Ok(is_sorted_primitive::<u8>(self)),
            DataType::UInt16 => Ok(is_sorted_primitive::<u16>(self)),
            DataType::UInt32 => Ok(is_sorted_primitive::<u32>(self)),
            DataType::UInt64 => Ok(is_sorted_primitive::<u64>(self)),
            DataType::Float32 => Ok(is_sorted_primitive::<f32>(self)),
            DataType::Float64 => Ok(is_sorted_primitive::<f64>(self)),
            _ => bail!("unsupported datatype: {:?}", self.data_type()),
        }
    }

    fn contains_duplicates(&self) -> anyhow::Result<bool> {
        debug_assert!(self.is_sorted()?);

        fn contains_duplicates_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            // Slices with less than 2 elements will yield 0 windows, which in turn will make
            // `all()` return `true`!
            (values.len() > 1)
                .then(|| values.values().windows(2).all(|v| v[0] == v[1]))
                .unwrap_or(false)
        }

        // TODO(cmc): support more datatypes as the need arise.
        match self.data_type() {
            DataType::Int8 => Ok(contains_duplicates_primitive::<i8>(self)),
            DataType::Int16 => Ok(contains_duplicates_primitive::<i16>(self)),
            DataType::Int32 => Ok(contains_duplicates_primitive::<i32>(self)),
            DataType::Int64 => Ok(contains_duplicates_primitive::<i64>(self)),
            DataType::UInt8 => Ok(contains_duplicates_primitive::<u8>(self)),
            DataType::UInt16 => Ok(contains_duplicates_primitive::<u16>(self)),
            DataType::UInt32 => Ok(contains_duplicates_primitive::<u32>(self)),
            DataType::UInt64 => Ok(contains_duplicates_primitive::<u64>(self)),
            DataType::Float32 => Ok(contains_duplicates_primitive::<f32>(self)),
            DataType::Float64 => Ok(contains_duplicates_primitive::<f64>(self)),
            _ => bail!("unsupported datatype: {:?}", self.data_type()),
        }
    }
}
