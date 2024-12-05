use std::fmt::{Display, Formatter};

/// A hue for a [`ColorToken`].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Hue {
    Gray,
    Green,
    Red,
    Blue,
    Purple,
}

impl nohash_hasher::IsEnabled for Hue {}

impl Hue {
    pub fn all() -> &'static [Self] {
        static ALL: [Hue; 5] = [Hue::Gray, Hue::Green, Hue::Red, Hue::Blue, Hue::Purple];
        &ALL
    }
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

/// A shade for a [`ColorToken`].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Shade {
    S0 = 0,
    S25 = 25,
    S50 = 50,
    S75 = 75,
    S100 = 100,
    S125 = 125,
    S150 = 150,
    S175 = 175,
    S200 = 200,
    S225 = 225,
    S250 = 250,
    S275 = 275,
    S300 = 300,
    S325 = 325,
    S350 = 350,
    S375 = 375,
    S400 = 400,
    S425 = 425,
    S450 = 450,
    S475 = 475,
    S500 = 500,
    S525 = 525,
    S550 = 550,
    S575 = 575,
    S600 = 600,
    S625 = 625,
    S650 = 650,
    S675 = 675,
    S700 = 700,
    S725 = 725,
    S750 = 750,
    S775 = 775,
    S800 = 800,
    S825 = 825,
    S850 = 850,
    S875 = 875,
    S900 = 900,
    S925 = 925,
    S950 = 950,
    S975 = 975,
    S1000 = 1000,
}

impl nohash_hasher::IsEnabled for Shade {}

impl Shade {
    pub fn all() -> &'static [Self] {
        static ALL: [Shade; 41] = [
            Shade::S0,
            Shade::S25,
            Shade::S50,
            Shade::S75,
            Shade::S100,
            Shade::S125,
            Shade::S150,
            Shade::S175,
            Shade::S200,
            Shade::S225,
            Shade::S250,
            Shade::S275,
            Shade::S300,
            Shade::S325,
            Shade::S350,
            Shade::S375,
            Shade::S400,
            Shade::S425,
            Shade::S450,
            Shade::S475,
            Shade::S500,
            Shade::S525,
            Shade::S550,
            Shade::S575,
            Shade::S600,
            Shade::S625,
            Shade::S650,
            Shade::S675,
            Shade::S700,
            Shade::S725,
            Shade::S750,
            Shade::S775,
            Shade::S800,
            Shade::S825,
            Shade::S850,
            Shade::S875,
            Shade::S900,
            Shade::S925,
            Shade::S950,
            Shade::S975,
            Shade::S1000,
        ];
        &ALL
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// A table mapping all combination of [`Hue`] and [`Shade`] to a [`egui::Color32`].
#[derive(Debug)]
pub struct ColorTable {
    pub color_table: nohash_hasher::IntMap<Hue, nohash_hasher::IntMap<Shade, egui::Color32>>,
}

impl ColorTable {
    #[inline]
    pub fn get(&self, token: ColorToken) -> egui::Color32 {
        *self
            .color_table
            .get(&token.hue)
            .and_then(|m| m.get(&token.shade))
            .expect("The color table must always be complete.")
    }

    #[inline]
    pub fn gray(&self, shade: Shade) -> egui::Color32 {
        self.get(ColorToken::gray(shade))
    }

    #[inline]
    pub fn green(&self, shade: Shade) -> egui::Color32 {
        self.get(ColorToken::green(shade))
    }

    #[inline]
    pub fn red(&self, shade: Shade) -> egui::Color32 {
        self.get(ColorToken::red(shade))
    }

    #[inline]
    pub fn blue(&self, shade: Shade) -> egui::Color32 {
        self.get(ColorToken::blue(shade))
    }

    #[inline]
    pub fn purple(&self, shade: Shade) -> egui::Color32 {
        self.get(ColorToken::purple(shade))
    }
}

/// A token representing a color in the [`ColorTable`].
///
/// Use [`crate::DesignTokens::color`] to get the color corresponding to a token.
#[derive(Debug, Clone, Copy, Hash)]
pub struct ColorToken {
    hue: Hue,
    shade: Shade,
}

impl ColorToken {
    #[inline]
    pub fn new(hue: Hue, shade: Shade) -> Self {
        Self { hue, shade }
    }

    #[inline]
    pub fn gray(shade: Shade) -> Self {
        Self::new(Hue::Gray, shade)
    }

    #[inline]
    pub fn green(shade: Shade) -> Self {
        Self::new(Hue::Green, shade)
    }

    #[inline]
    pub fn red(shade: Shade) -> Self {
        Self::new(Hue::Red, shade)
    }

    #[inline]
    pub fn blue(shade: Shade) -> Self {
        Self::new(Hue::Blue, shade)
    }

    #[inline]
    pub fn purple(shade: Shade) -> Self {
        Self::new(Hue::Purple, shade)
    }
}
