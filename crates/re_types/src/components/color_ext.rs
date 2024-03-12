use super::Color;

impl Color {
    pub const BLACK: Self = Self(crate::datatypes::Rgba32::BLACK);
    pub const WHITE: Self = Self(crate::datatypes::Rgba32::WHITE);
    pub const TRANSPARENT: Self = Self(crate::datatypes::Rgba32::TRANSPARENT);

    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from([r, g, b, 255])
    }

    #[inline]
    pub fn from_unmultiplied_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from(crate::datatypes::Rgba32::from_unmultiplied_rgba(r, g, b, a))
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub fn from_u32(rgba: u32) -> Self {
        Self(rgba.into())
    }

    /// `[r, g, b, a]`
    #[inline]
    pub fn to_array(self) -> [u8; 4] {
        [
            (self.0 .0 >> 24) as u8,
            (self.0 .0 >> 16) as u8,
            (self.0 .0 >> 8) as u8,
            self.0 .0 as u8,
        ]
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub fn to_u32(self) -> u32 {
        self.0 .0
    }
}

impl Color {
    #[inline]
    pub fn new(value: impl Into<crate::datatypes::Rgba32>) -> Self {
        Self(value.into())
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Color32 {
    fn from(color: Color) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_unmultiplied(r, g, b, a)
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Rgba {
    fn from(color: Color) -> Self {
        let color: ecolor::Color32 = color.into();
        color.into()
    }
}
