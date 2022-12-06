use crate::texture_values::ValueRgba8UnormSrgb;

/// A 32 bit color, 8 channel per color, RGB in (gamma) srgb space, premultiplied with alpha value.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorRgba8SrgbPremultiplied(pub ValueRgba8UnormSrgb);

impl ColorRgba8SrgbPremultiplied {
    pub const WHITE: Self = Self(ValueRgba8UnormSrgb::WHITE);
}

impl From<[u8; 4]> for ColorRgba8SrgbPremultiplied {
    #[inline]
    fn from(array: [u8; 4]) -> ColorRgba8SrgbPremultiplied {
        Self(array.into())
    }
}
