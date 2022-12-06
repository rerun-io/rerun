//! Defines structs and conversions between them as they are expected to be done by [`wgpu::TextureFormat`]
//!
//! These data types are meant to behave like wgpu textures!
//! With the exception of srgb conversion enforced by srgb post-fixed formats, this does not define any color space semantics.

/// A pixel in [`wgpu::TextureFormat::Rgba8UnormSrgb`]
///
/// It has 8 bit per channel and it's assumed to be in srgb gamma color space.
/// Conversions from and to non-srgb types require a conversion.
/// It does *not* specify whether alpha is pre-multiplied or not.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ValueRgba8UnormSrgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ValueRgba8UnormSrgb {
    pub const WHITE: ValueRgba8UnormSrgb = ValueRgba8UnormSrgb {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    pub const TRANSPARENT: ValueRgba8UnormSrgb = ValueRgba8UnormSrgb {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
}

impl From<ValueRgba8UnormSrgb> for wgpu::Color {
    #[inline]
    fn from(rgba: ValueRgba8UnormSrgb) -> Self {
        ValueRgba32Float::from(rgba).into()
    }
}

impl From<[u8; 4]> for ValueRgba8UnormSrgb {
    #[inline]
    fn from(array: [u8; 4]) -> ValueRgba8UnormSrgb {
        Self {
            r: array[0],
            g: array[1],
            b: array[2],
            a: array[3],
        }
    }
}

/// A pixel in [`wgpu::TextureFormat::Rgba32Float`]
///
/// It has 32 bit floats per channel but its semantics are not known!
/// (It does *not* specify color space or whether alpha pre-multiplied or not)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ValueRgba32Float {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<ValueRgba32Float> for wgpu::Color {
    #[inline]
    fn from(rgbaf: ValueRgba32Float) -> wgpu::Color {
        wgpu::Color {
            r: rgbaf.r as f64,
            g: rgbaf.g as f64,
            b: rgbaf.b as f64,
            a: rgbaf.a as f64,
        }
    }
}

impl From<ValueRgba8UnormSrgb> for ValueRgba32Float {
    #[inline]
    fn from(srgb8: ValueRgba8UnormSrgb) -> ValueRgba32Float {
        let srgb = glam::vec3(srgb8.r as f32, srgb8.g as f32, srgb8.b as f32) / 255.0;
        let cutoff = (srgb - glam::vec3(0.04045, 0.04045, 0.04045)).ceil();
        let under = srgb / 12.92;
        let over = ((srgb + glam::vec3(0.055, 0.055, 0.055)) / 1.055).powf(2.4);
        let rgb = under + (over - under) * cutoff;
        ValueRgba32Float {
            r: rgb.x,
            g: rgb.y,
            b: rgb.z,
            a: srgb8.a as f32 / 255.0,
        }
    }
}
