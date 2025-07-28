pub mod sensor_msgs;
mod unsupported;

pub use unsupported::UnsupportedSchemaMessageParser;

pub(crate) fn fixed_size_list_builder<T: arrow::array::ArrayBuilder + Default>(
    value_length: i32,
    capacity: usize,
) -> arrow::array::FixedSizeListBuilder<T> {
    arrow::array::FixedSizeListBuilder::with_capacity(Default::default(), value_length, capacity)
}
