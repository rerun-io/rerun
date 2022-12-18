use arrow2::{
    array::{Array, PrimitiveArray},
    datatypes::DataType,
    types::NativeType,
};

// ---

/// Returns `true` if the given array is dense.
pub fn is_dense_array(arr: &dyn Array) -> bool {
    arr.validity().is_none()
}

/// Returns `true` if the given array is sorted increasingly.
pub fn is_sorted_array(arr: &dyn Array) -> bool {
    fn is_sorted_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
        let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
        values.values().windows(2).all(|v| v[0] <= v[1])
    }

    // TODO(cmc): support more datatypes as the need arise.
    #[allow(clippy::todo)]
    match arr.data_type() {
        DataType::Null => todo!("unsupported datatype"),
        DataType::Boolean => todo!("unsupported datatype"),
        DataType::Int8 => is_sorted_primitive::<i8>(arr),
        DataType::Int16 => is_sorted_primitive::<i16>(arr),
        DataType::Int32 => is_sorted_primitive::<i32>(arr),
        DataType::Int64 => is_sorted_primitive::<i64>(arr),
        DataType::UInt8 => is_sorted_primitive::<u8>(arr),
        DataType::UInt16 => is_sorted_primitive::<u16>(arr),
        DataType::UInt32 => is_sorted_primitive::<u32>(arr),
        DataType::UInt64 => is_sorted_primitive::<u64>(arr),
        DataType::Float16 => todo!("unsupported datatype"),
        DataType::Float32 => is_sorted_primitive::<f32>(arr),
        DataType::Float64 => is_sorted_primitive::<f64>(arr),
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
