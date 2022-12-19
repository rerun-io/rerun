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
    fn is_sorted(&self) -> bool;
}

impl ArrayExt for dyn Array {
    fn is_dense(&self) -> bool {
        self.validity().is_none()
    }

    fn is_sorted(&self) -> bool {
        fn is_sorted_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            values.values().windows(2).all(|v| v[0] <= v[1])
        }

        // TODO(cmc): support more datatypes as the need arise.
        #[allow(clippy::todo)]
        match self.data_type() {
            DataType::Null => todo!("unsupported datatype"),
            DataType::Boolean => todo!("unsupported datatype"),
            DataType::Int8 => is_sorted_primitive::<i8>(self),
            DataType::Int16 => is_sorted_primitive::<i16>(self),
            DataType::Int32 => is_sorted_primitive::<i32>(self),
            DataType::Int64 => is_sorted_primitive::<i64>(self),
            DataType::UInt8 => is_sorted_primitive::<u8>(self),
            DataType::UInt16 => is_sorted_primitive::<u16>(self),
            DataType::UInt32 => is_sorted_primitive::<u32>(self),
            DataType::UInt64 => is_sorted_primitive::<u64>(self),
            DataType::Float16 => todo!("unsupported datatype"),
            DataType::Float32 => is_sorted_primitive::<f32>(self),
            DataType::Float64 => is_sorted_primitive::<f64>(self),
            DataType::Timestamp(_, _) => todo!("unsupported datatype"),
            DataType::Date32 => todo!("unsupported datatype"),
            DataType::Date64 => todo!("unsupported datatype"),
            DataType::Time32(_) => todo!("unsupported datatype"),
            DataType::Time64(_) => todo!("unsupported datatype"),
            DataType::Duration(_) => todo!("unsupported datatype"),
            DataType::Interval(_) => todo!("unsupported datatype"),
            DataType::Binary => todo!("unsupported datatype"),
            DataType::FixedSizeBinary(_) => todo!("unsupported datatype"),
            DataType::LargeBinary => todo!("unsupported datatype"),
            DataType::Utf8 => todo!("unsupported datatype"),
            DataType::LargeUtf8 => todo!("unsupported datatype"),
            DataType::List(_) => todo!("unsupported datatype"),
            DataType::FixedSizeList(_, _) => todo!("unsupported datatype"),
            DataType::LargeList(_) => todo!("unsupported datatype"),
            DataType::Struct(_) => todo!("unsupported datatype"),
            DataType::Union(_, _, _) => todo!("unsupported datatype"),
            DataType::Map(_, _) => todo!("unsupported datatype"),
            DataType::Dictionary(_, _, _) => todo!("unsupported datatype"),
            DataType::Decimal(_, _) => todo!("unsupported datatype"),
            DataType::Decimal256(_, _) => todo!("unsupported datatype"),
            DataType::Extension(_, _, _) => todo!("unsupported datatype"),
        }
    }
}
