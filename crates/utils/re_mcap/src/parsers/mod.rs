pub mod cdr;
pub mod dds;
mod decode;
pub mod ros2msg;

pub use decode::{ChannelId, MessageParser, ParserContext};

/// Defines utility functions shared across parsers.
pub(crate) mod util {
    use arrow::{
        array::{FixedSizeListBuilder, ListBuilder, UInt8Builder},
        datatypes::{DataType, Field},
    };
    use re_types::{Loggable as _, components};
    use std::sync::Arc;

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
