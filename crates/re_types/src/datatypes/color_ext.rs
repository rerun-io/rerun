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
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }
}

impl From<u32> for Color {
    #[inline]
    fn from(c: u32) -> Self {
        Self(c)
    }
}

impl From<crate::components::Color> for Color {
    #[inline]
    fn from(c: crate::components::Color) -> Self {
        Self(c.0)
    }
}

impl From<[u8; 4]> for Color {
    #[inline]
    fn from(bytes: [u8; 4]) -> Self {
        Self(
            (bytes[0] as u32) << 24
                | (bytes[1] as u32) << 16
                | (bytes[2] as u32) << 8
                | (bytes[3] as u32),
        )
    }
}

#[cfg(feature = "ecolor")]
impl From<Color> for ecolor::Color32 {
    fn from(color: Color) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_premultiplied(r, g, b, a)
    }
}
