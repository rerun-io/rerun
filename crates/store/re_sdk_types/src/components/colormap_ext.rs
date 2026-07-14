use super::Colormap;
use crate::ColormapCategory;

impl Colormap {
    /// Returns the [`ColormapCategory`] classification for this colormap.
    pub fn category(&self) -> ColormapCategory {
        ColormapCategory::from_colormap(*self)
    }
}
