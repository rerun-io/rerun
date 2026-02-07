use crate::components::Colormap;

/// A category classification for colormaps based on their visual progression.
///
/// This is *not* a component, but a helper type for classifying [`crate::components::Colormap`] variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColormapCategory {
    /// Colormaps that progress from one color to another in a single direction.
    Sequential,

    /// Colormaps that transition between two contrasting colors, often with a neutral midpoint.
    Diverging,

    /// Colormaps that wrap around.
    Cyclic,
}

impl ColormapCategory {
    /// Returns all possible colormap categories.
    pub fn variants() -> &'static [Self] {
        &[Self::Sequential, Self::Diverging, Self::Cyclic]
    }

    /// Returns the [`ColormapCategory`] classification for the given colormap.
    pub fn from_colormap(colormap: Colormap) -> Self {
        match colormap {
            Colormap::Grayscale
            | Colormap::Inferno
            | Colormap::Magma
            | Colormap::Plasma
            | Colormap::Viridis
            | Colormap::Turbo => Self::Sequential,
            Colormap::CyanToYellow | Colormap::Spectral => Self::Diverging,
            Colormap::Twilight => Self::Cyclic,
        }
    }
}
