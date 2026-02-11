//! Library providing utilities to load MCAP files with Rerun.

mod error;
pub mod layers;

pub(crate) mod parsers;
pub(crate) mod util;

pub use error::Error;
pub use layers::{Layer, LayerIdentifier, LayerRegistry, MessageLayer, SelectedLayers};
pub use parsers::ros2msg::sensor_msgs::{ImageEncoding, decode_image_format};
pub use parsers::{MessageParser, ParserContext, cdr};
// TODO(grtlr): We should expose an `Mcap` object that internally holds the summary + a reference to the bytes.
pub use util::read_summary;
