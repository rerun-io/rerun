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

    /// Returns `true` if the array is both sorted (increasing order) and contains only unique
    /// values.
    ///
    /// The array must be dense, otherwise the result of this method is undefined.
    fn is_sorted_and_unique(&self) -> anyhow::Result<bool>;
}

impl ArrayExt for dyn Array {
    fn is_dense(&self) -> bool {
        if let Some(validity) = self.validity() {
            validity.iter().all(|b| b)
        } else {
            true
        }
    }

    fn is_sorted_and_unique(&self) -> anyhow::Result<bool> {
        debug_assert!(self.is_dense());

        fn is_sorted_and_unique_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            values.values().windows(2).all(|v| v[0] < v[1])
        }

        // TODO(cmc): support more datatypes as the need arise.
        match self.data_type() {
            DataType::Int8 => Ok(is_sorted_and_unique_primitive::<i8>(self)),
            DataType::Int16 => Ok(is_sorted_and_unique_primitive::<i16>(self)),
            DataType::Int32 => Ok(is_sorted_and_unique_primitive::<i32>(self)),
            DataType::Int64 => Ok(is_sorted_and_unique_primitive::<i64>(self)),
            DataType::UInt8 => Ok(is_sorted_and_unique_primitive::<u8>(self)),
            DataType::UInt16 => Ok(is_sorted_and_unique_primitive::<u16>(self)),
            DataType::UInt32 => Ok(is_sorted_and_unique_primitive::<u32>(self)),
            DataType::UInt64 => Ok(is_sorted_and_unique_primitive::<u64>(self)),
            DataType::Float32 => Ok(is_sorted_and_unique_primitive::<f32>(self)),
            DataType::Float64 => Ok(is_sorted_and_unique_primitive::<f64>(self)),
            _ => bail!("unsupported datatype: {:?}", self.data_type()),
        }
    }
}
