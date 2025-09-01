pub mod cdr;
pub(crate) mod dds;
mod decode;
pub mod ros2msg;

pub use decode::{ChannelId, MessageParser, ParserContext};

/// Defines utility functions shared across parsers.
pub(crate) mod util {
    use arrow::array::{ArrayBuilder, FixedSizeListBuilder};

    pub(crate) fn fixed_size_list_builder<T: ArrayBuilder + Default>(
        value_length: i32,
        capacity: usize,
    ) -> FixedSizeListBuilder<T> {
        FixedSizeListBuilder::with_capacity(Default::default(), value_length, capacity)
    }
}
