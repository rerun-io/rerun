use super::Color;

impl Color {
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from([r, g, b, 255])
    }

    #[inline]
    pub fn from_unmultiplied_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from([r, g, b, a])
    }

    #[inline]
    pub fn to_array(self) -> [u8; 4] {
        [
            (self.0 .0 >> 24) as u8,
            (self.0 .0 >> 16) as u8,
            (self.0 .0 >> 8) as u8,
            self.0 .0 as u8,
        ]
    }
}

impl Color {
    #[inline]
    pub fn new(value: impl Into<crate::datatypes::Color>) -> Self {
        Self(value.into())
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Color32 {
    fn from(color: Color) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_premultiplied(r, g, b, a)
    }
}
