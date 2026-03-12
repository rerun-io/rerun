//! Library providing utilities to load MCAP files with Rerun.

pub mod decoders;
mod error;

pub(crate) mod parsers;
pub(crate) mod util;

pub use decoders::{Decoder, DecoderIdentifier, DecoderRegistry, MessageDecoder, SelectedDecoders};
pub use error::Error;
pub use parsers::ros2msg::sensor_msgs::{
    ImageEncoding, decode_image_encoding, decode_image_format,
};
pub use parsers::{MessageParser, ParserContext, cdr};
// TODO(grtlr): We should expose an `Mcap` object that internally holds the summary + a reference to the bytes.
pub use util::read_summary;
