#![allow(clippy::iter_over_hash_type)]

//! Library providing utilities to load MCAP files with Rerun.

/// Every MCAP record is framed by a fixed header containing a one-byte opcode followed by an
/// eight-byte little-endian `u64` body length.
const RECORD_HEADER_LEN: usize = 1 + std::mem::size_of::<u64>();

pub mod decoders;
mod error;
mod file;
mod info;
mod recover;

pub(crate) mod parsers;
pub(crate) mod util;

pub use decoders::{
    Decoder, DecoderIdentifier, DecoderRegistry, MessageDecoder, SelectedDecoders, TopicFilter,
};

pub use error::Error;
pub use file::McapFile;
pub use info::{
    McapChannelInfo, McapChunkInfo, McapCompressionInfo, McapInfo, McapSchemaInfo,
    McapSummarySource,
};
pub use mcap::Summary;
pub use parsers::ros2msg::sensor_msgs::{
    ImageEncoding, decode_image_encoding, decode_image_format,
};
pub use parsers::{MessageParser, ParserContext, cdr};
pub use recover::{ScanResult, build_chunk_index, read_or_reconstruct_summary};
pub use util::read_summary;
