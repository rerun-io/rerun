use super::Color;

impl Color {
    /// Black and opaque.
    pub const BLACK: Self = Self(crate::datatypes::Rgba32::BLACK);

    /// White and opaque.
    pub const WHITE: Self = Self(crate::datatypes::Rgba32::WHITE);

    /// Fully transparent (invisible).
    pub const TRANSPARENT: Self = Self(crate::datatypes::Rgba32::TRANSPARENT);

    /// From gamma-space sRGB values.
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from([r, g, b, 255])
    }

    /// From gamma-space sRGB values, with a separate/unmultiplied alpha in linear-space.
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
            (self.0.0 >> 24) as u8,
            (self.0.0 >> 16) as u8,
            (self.0.0 >> 8) as u8,
            self.0.0 as u8,
        ]
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub fn to_u32(self) -> u32 {
        self.0.0
    }
}

impl Color {
    /// Create a new color.
    #[inline]
    pub fn new(value: impl Into<crate::datatypes::Rgba32>) -> Self {
        Self(value.into())
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Color32 {
    fn from(color: Color) -> Self {
        let [r, g, b, a] = color.to_array();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
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

impl Default for Color {
    #[inline]
    fn default() -> Self {
        // Pretty hard to pick a good default value.
        // White is best since multiplicative it does nothing and is visible in more cases than black would be.
        // Most of the time, the `FallbackProviderRegistry` should provide a better value.
        Self::WHITE
    }
}
