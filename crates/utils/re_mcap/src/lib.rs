//! Library providing utilities to load MCAP files with Rerun.

mod layers;

pub(crate) mod parsers;
pub(crate) mod util;

pub use layers::{Layer, LayerIdentifier, LayerRegistry, MessageLayer, SelectedLayers};
// TODO(grtlr): We should expose an `Mcap` object that internally holds the summary + a reference to the bytes.
pub use util::read_summary;
