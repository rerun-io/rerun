mod foxglove;
mod helpers;
mod image_helpers;

pub use foxglove::foxglove_lenses;

/// The identifier used to enable/disable Foxglove lenses via [`re_mcap::SelectedLayers`].
pub const FOXGLOVE_LENSES_IDENTIFIER: &str = "foxglove";
