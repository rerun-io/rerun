/// RGBA color in sRGB gamma space, with separate/unmultiplied linear alpha.
///
/// This is the most common input color, e.g. specified using CSS colors.
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

/// [`ecolor::Color32`] but without `repr(align(4))`, since that is not compatible
/// with `repr(packed)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct UnalignedColor32(pub [u8; 4]);

impl From<ecolor::Color32> for UnalignedColor32 {
    fn from(c: ecolor::Color32) -> Self {
        Self(c.to_array())
    }
}

impl From<UnalignedColor32> for ecolor::Color32 {
    fn from(c: UnalignedColor32) -> Self {
        let [r, g, b, a] = c.0;

        // We aren't hard-coding any colors here.
        #[expect(clippy::disallowed_methods)]
        Self::from_rgba_premultiplied(r, g, b, a)
    }
}
