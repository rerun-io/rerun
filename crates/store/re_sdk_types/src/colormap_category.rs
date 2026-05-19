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

    /// Colormaps specialized for occupancy grids and costmaps.
    GridMap,
}

/// Allows to select groups of colormap categories.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColormapSelection {
    #[default]
    /// Show the standard colormap categories.
    Standard,

    /// Show the standard colormap categories plus GridMap-specific colormaps.
    IncludeGridMap,
}

impl ColormapSelection {
    /// Whether this selection includes the given category.
    pub const fn includes(self, category: ColormapCategory) -> bool {
        match self {
            Self::Standard => !matches!(category, ColormapCategory::GridMap),
            Self::IncludeGridMap => true,
        }
    }
}

impl ColormapCategory {
    /// Returns all possible colormap categories.
    pub fn variants() -> &'static [Self] {
        &[
            Self::Sequential,
            Self::Diverging,
            Self::Cyclic,
            Self::GridMap,
        ]
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
            Colormap::RvizMap | Colormap::RvizCostmap => Self::GridMap,
        }
    }
}
