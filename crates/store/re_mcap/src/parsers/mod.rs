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
    use re_sdk_types::{ArrowDatatype as _, components};

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
        // The bytes of a blob are always present, matching `components::Blob::arrow_datatype()`.
        let list_builder = ListBuilder::<UInt8Builder>::default()
            .with_field(Arc::new(Field::new_list_field(DataType::UInt8, false)));

        // The outer list is the per-row component list of a chunk column, so it has to follow the
        // canonical form, like everywhere else in Rerun. See
        // <https://github.com/rerun-io/rerun/issues/12887> for a case where this was violated.
        FixedSizeListBuilder::with_capacity(list_builder, 1, capacity).with_field(Arc::new(
            re_arrow_util::canonical_component_list_field(components::Blob::arrow_datatype()),
        ))
    }
}
