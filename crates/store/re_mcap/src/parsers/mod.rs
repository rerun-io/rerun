pub mod cdr;
pub(crate) mod dds;
mod decode;
pub(crate) mod ros2msg;

pub use decode::{ChannelId, MessageParser, ParserContext};

/// Defines utility functions shared across parsers.
pub(crate) mod util {
    use std::sync::Arc;

    use arrow::array::{FixedSizeListBuilder, ListBuilder, UInt8Builder};
    use arrow::datatypes::{DataType, Field};
    use re_sdk_types::{Loggable as _, components};

    pub(crate) fn fixed_size_list_builder<T: arrow::array::ArrayBuilder + Default>(
        value_length: i32,
        capacity: usize,
    ) -> arrow::array::FixedSizeListBuilder<T> {
        arrow::array::FixedSizeListBuilder::with_capacity(
            Default::default(),
            value_length,
            capacity,
        )
    }

    pub(crate) fn blob_list_builder(
        capacity: usize,
    ) -> FixedSizeListBuilder<ListBuilder<UInt8Builder>> {
        let list_builder = ListBuilder::<UInt8Builder>::default()
            .with_field(Arc::new(Field::new_list_field(DataType::UInt8, false)));

        FixedSizeListBuilder::with_capacity(list_builder, 1, capacity).with_field(Arc::new(
            Field::new_list_field(components::Blob::arrow_datatype(), false),
        ))
    }
}
