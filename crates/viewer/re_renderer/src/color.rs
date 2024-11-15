/// RGBA color in sRGB gamma space, with separate/unmultiplied linear alpha.
///
/// This is the most common input color, e.g. speicied using CSS colors.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgba32Unmul(pub [u8; 4]);

impl Rgba32Unmul {
    pub const BLACK: Self = Self([0, 0, 0, 255]);
    pub const WHITE: Self = Self([255, 255, 255, 255]);
    pub const TRANSPARENT: Self = Self([0, 0, 0, 0]);

    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    #[inline]
    pub fn from_rgba_unmul_array(rgba_unmul: [u8; 4]) -> Self {
        Self(rgba_unmul)
    }
}
