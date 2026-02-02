use super::Rgba32;

impl Rgba32 {
    /// Black and opaque.
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);

    /// White and opaque.
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    /// Fully transparent (invisible).
    pub const TRANSPARENT: Self = Self::from_unmultiplied_rgba(0, 0, 0, 0);

    /// From gamma-space sRGB values.
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_unmultiplied_rgba(r, g, b, 255)
    }

    /// From gamma-space sRGB values, with a separate/unmultiplied alpha in linear-space.
    #[inline]
    pub const fn from_unmultiplied_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let [r, g, b, a] = [r as u32, g as u32, b as u32, a as u32];
        Self((r << 24) | (g << 16) | (b << 8) | a)
    }

    /// From linear-space sRGB values in 0-1 range, with a separate/unmultiplied alpha.
    ///
    /// This is a lossy conversion.
    #[cfg(feature = "ecolor")]
    pub fn from_linear_unmultiplied_rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        ecolor::Rgba::from_rgba_unmultiplied(r, g, b, a).into()
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub const fn from_u32(rgba: u32) -> Self {
        Self(rgba)
    }

    /// `[r, g, b, a]`
    #[inline]
    pub const fn to_array(self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }

    /// Most significant byte is `r`, least significant byte is `a`.
    #[inline]
    pub const fn to_u32(self) -> u32 {
        self.0
    }
}

impl From<(u8, u8, u8)> for Rgba32 {
    #[inline]
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::from_rgb(r, g, b)
    }
}

impl From<[u8; 3]> for Rgba32 {
    #[inline]
    fn from([r, g, b]: [u8; 3]) -> Self {
        Self::from_rgb(r, g, b)
    }
}

impl From<[u8; 4]> for Rgba32 {
    #[inline]
    fn from([r, g, b, a]: [u8; 4]) -> Self {
        Self::from_unmultiplied_rgba(r, g, b, a)
    }
}

impl From<(u8, u8, u8, u8)> for Rgba32 {
    #[inline]
    fn from((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self::from_unmultiplied_rgba(r, g, b, a)
    }
}

#[cfg(feature = "ecolor")]
impl From<Rgba32> for ecolor::Color32 {
    fn from(color: Rgba32) -> Self {
        let [r, g, b, a] = color.to_array();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        Self::from_rgba_unmultiplied(r, g, b, a)
    }
}

#[cfg(feature = "ecolor")]
impl From<Rgba32> for ecolor::Rgba {
    fn from(color: Rgba32) -> Self {
        let color: ecolor::Color32 = color.into();
        color.into()
    }
}

#[cfg(feature = "ecolor")]
impl From<ecolor::Rgba> for Rgba32 {
    fn from(val: ecolor::Rgba) -> Self {
        val.to_srgba_unmultiplied().into()
    }
}

#[cfg(feature = "ecolor")]
impl From<ecolor::Color32> for Rgba32 {
    fn from(val: ecolor::Color32) -> Self {
        val.to_srgba_unmultiplied().into()
    }
}
