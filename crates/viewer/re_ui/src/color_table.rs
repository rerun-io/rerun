use std::fmt::{Display, Formatter};

use strum::{EnumCount, EnumIter, IntoEnumIterator};

/// A hue for a [`ColorToken`].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EnumIter, EnumCount)]
pub enum Hue {
    Gray,
    Green,
    Red,
    Blue,
    Purple,
}

impl Display for Hue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            // these must be as they appear in `design_token.json`
            Self::Gray => f.write_str("Gray"),
            Self::Green => f.write_str("Green"),
            Self::Red => f.write_str("Red"),
            Self::Blue => f.write_str("Blue"),
            Self::Purple => f.write_str("Purple"),
        }
    }
}

/// A color scale for a [`ColorToken`].
///
/// A scale is an arbitrary… well… scale of subjective color "intensity". Both brightness and
/// saturation may vary along the scale. For a dark mode theme, low scales are typically darker and
/// used for backgrounds, whereas high scales are typically brighter and used for text and
/// interactive UI elements.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EnumIter, EnumCount)]
pub enum Scale {
    S0,
    S25,
    S50,
    S75,
    S100,
    S125,
    S150,
    S175,
    S200,
    S225,
    S250,
    S275,
    S300,
    S325,
    S350,
    S375,
    S400,
    S425,
    S450,
    S475,
    S500,
    S525,
    S550,
    S575,
    S600,
    S625,
    S650,
    S675,
    S700,
    S725,
    S750,
    S775,
    S800,
    S825,
    S850,
    S875,
    S900,
    S925,
    S950,
    S975,
    S1000,
}

impl Display for Scale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let txt = match self {
            Self::S0 => "0",
            Self::S25 => "25",
            Self::S50 => "50",
            Self::S75 => "75",
            Self::S100 => "100",
            Self::S125 => "125",
            Self::S150 => "150",
            Self::S175 => "175",
            Self::S200 => "200",
            Self::S225 => "225",
            Self::S250 => "250",
            Self::S275 => "275",
            Self::S300 => "300",
            Self::S325 => "325",
            Self::S350 => "350",
            Self::S375 => "375",
            Self::S400 => "400",
            Self::S425 => "425",
            Self::S450 => "450",
            Self::S475 => "475",
            Self::S500 => "500",
            Self::S525 => "525",
            Self::S550 => "550",
            Self::S575 => "575",
            Self::S600 => "600",
            Self::S625 => "625",
            Self::S650 => "650",
            Self::S675 => "675",
            Self::S700 => "700",
            Self::S725 => "725",
            Self::S750 => "750",
            Self::S775 => "775",
            Self::S800 => "800",
            Self::S825 => "825",
            Self::S850 => "850",
            Self::S875 => "875",
            Self::S900 => "900",
            Self::S925 => "925",
            Self::S950 => "950",
            Self::S975 => "975",
            Self::S1000 => "1000",
        };

        txt.fmt(f)
    }
}

/// A table mapping all combination of [`Hue`] and [`Scale`] to a [`egui::Color32`].
#[derive(Debug)]
pub struct ColorTable {
    color_lut: Vec<Vec<egui::Color32>>,
}

impl ColorTable {
    /// Build a new color table by calling the provided closure with all possible entries.
    pub fn new(mut color_lut_fn: impl FnMut(ColorToken) -> egui::Color32) -> Self {
        Self {
            color_lut: Hue::iter()
                .map(|hue| {
                    Scale::iter()
                        .map(|scale| color_lut_fn(ColorToken::new(hue, scale)))
                        .collect()
                })
                .collect(),
        }
    }

    #[inline]
    pub fn get(&self, token: ColorToken) -> egui::Color32 {
        self.color_lut[token.hue as usize][token.scale as usize]
    }

    #[inline]
    pub fn gray(&self, shade: Scale) -> egui::Color32 {
        self.get(ColorToken::gray(shade))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn green(&self, shade: Scale) -> egui::Color32 {
        self.get(ColorToken::green(shade))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn red(&self, shade: Scale) -> egui::Color32 {
        self.get(ColorToken::red(shade))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn blue(&self, shade: Scale) -> egui::Color32 {
        self.get(ColorToken::blue(shade))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn purple(&self, shade: Scale) -> egui::Color32 {
        self.get(ColorToken::purple(shade))
    }
}

/// A token representing a color in the global color table.
///
/// Use [`crate::DesignTokens::color`] to get the color corresponding to a token.
#[derive(Debug, Clone, Copy, Hash)]
pub struct ColorToken {
    pub hue: Hue,
    pub scale: Scale,
}

impl ColorToken {
    #[inline]
    pub fn new(hue: Hue, shade: Scale) -> Self {
        Self { hue, scale: shade }
    }

    #[inline]
    pub fn gray(shade: Scale) -> Self {
        Self::new(Hue::Gray, shade)
    }

    #[inline]
    pub fn green(shade: Scale) -> Self {
        Self::new(Hue::Green, shade)
    }

    #[inline]
    pub fn red(shade: Scale) -> Self {
        Self::new(Hue::Red, shade)
    }

    #[inline]
    pub fn blue(shade: Scale) -> Self {
        Self::new(Hue::Blue, shade)
    }

    #[inline]
    pub fn purple(shade: Scale) -> Self {
        Self::new(Hue::Purple, shade)
    }
}
