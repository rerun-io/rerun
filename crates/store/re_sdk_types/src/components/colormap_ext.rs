use super::Colormap;
use crate::ColormapCategory;

impl Colormap {
    /// Instantiate a new [`Colormap`] from a u8 value.
    ///
    /// Returns `None` if the value doesn't match any of the enum's arms.
    pub fn from_u8(value: u8) -> Option<Self> {
        // NOTE: This code will be optimized out, it's only here to make sure this method fails to
        // compile if the enum is modified.
        match Self::default() {
            Self::Grayscale
            | Self::Inferno
            | Self::Magma
            | Self::Plasma
            | Self::Turbo
            | Self::Viridis
            | Self::CyanToYellow
            | Self::Spectral
            | Self::Twilight => {}
        }

        match value {
            v if v == Self::Grayscale as u8 => Some(Self::Grayscale),
            v if v == Self::Inferno as u8 => Some(Self::Inferno),
            v if v == Self::Magma as u8 => Some(Self::Magma),
            v if v == Self::Plasma as u8 => Some(Self::Plasma),
            v if v == Self::Turbo as u8 => Some(Self::Turbo),
            v if v == Self::Viridis as u8 => Some(Self::Viridis),
            v if v == Self::CyanToYellow as u8 => Some(Self::CyanToYellow),
            v if v == Self::Spectral as u8 => Some(Self::Spectral),
            v if v == Self::Twilight as u8 => Some(Self::Twilight),
            _ => None,
        }
    }

    /// Returns the [`ColormapCategory`] classification for this colormap.
    pub fn category(&self) -> ColormapCategory {
        ColormapCategory::from_colormap(*self)
    }
}
