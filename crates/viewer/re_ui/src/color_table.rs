use std::fmt::{Display, Formatter};
use std::str::FromStr;

use strum::{EnumCount, EnumIter, IntoEnumIterator as _};

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

impl FromStr for Hue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Gray" => Self::Gray,
            "Green" => Self::Green,
            "Red" => Self::Red,
            "Blue" => Self::Blue,
            "Purple" => Self::Purple,
            _ => return Err(anyhow::anyhow!("Invalid hue: {s:?}")),
        })
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

impl FromStr for Scale {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "0" => Self::S0,
            "25" => Self::S25,
            "50" => Self::S50,
            "75" => Self::S75,
            "100" => Self::S100,
            "125" => Self::S125,
            "150" => Self::S150,
            "175" => Self::S175,
            "200" => Self::S200,
            "225" => Self::S225,
            "250" => Self::S250,
            "275" => Self::S275,
            "300" => Self::S300,
            "325" => Self::S325,
            "350" => Self::S350,
            "375" => Self::S375,
            "400" => Self::S400,
            "425" => Self::S425,
            "450" => Self::S450,
            "475" => Self::S475,
            "500" => Self::S500,
            "525" => Self::S525,
            "550" => Self::S550,
            "575" => Self::S575,
            "600" => Self::S600,
            "625" => Self::S625,
            "650" => Self::S650,
            "675" => Self::S675,
            "700" => Self::S700,
            "725" => Self::S725,
            "750" => Self::S750,
            "775" => Self::S775,
            "800" => Self::S800,
            "825" => Self::S825,
            "850" => Self::S850,
            "875" => Self::S875,
            "900" => Self::S900,
            "925" => Self::S925,
            "950" => Self::S950,
            "975" => Self::S975,
            "1000" => Self::S1000,
            _ => return Err(anyhow::anyhow!("Invalid scale: {s:?}")),
        })
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
}

/// A token representing a color in the global color table.
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
}
