use super::Color;

impl Color {
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from([r, g, b, 255])
    }

    #[inline]
    pub fn from_unmultiplied_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let [r, g, b, a] = [r as u32, g as u32, b as u32, a as u32];
        Self(r << 24 | g << 16 | b << 8 | a)
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub fn from_u32(rgba: u32) -> Self {
        Self(rgba)
    }

    /// `[r, g, b, a]`
    #[inline]
    pub fn to_array(self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub fn to_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for Color {
    #[inline]
    fn from(rgba: u32) -> Self {
        Self::from_u32(rgba)
    }
}

impl From<[u8; 4]> for Color {
    #[inline]
    fn from([r, g, b, a]: [u8; 4]) -> Self {
        Self::from_unmultiplied_rgba(r, g, b, a)
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Color32 {
    fn from(color: Color) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_unmultiplied(r, g, b, a)
    }
}
