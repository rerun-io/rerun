//! Explicitly padded and/or aligned types following wgsl rules.
//! See [wgsl spec on alignment and size](https://www.w3.org/TR/WGSL/#alignment-and-size)
//!
//! This is especially important for cases where [`glam`] isn't explicit about padding and alignment.

use bytemuck::{Pod, Zeroable};

// --- Vec ---

#[repr(C, align(8))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl From<glam::Vec2> for Vec2 {
    #[inline]
    fn from(v: glam::Vec2) -> Self {
        Vec2 { x: v.x, y: v.y }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec2Padded {
    pub x: f32,
    pub y: f32,
    pub padding0: f32,
    pub padding1: f32,
}

impl From<glam::Vec2> for Vec2Padded {
    #[inline]
    fn from(v: glam::Vec2) -> Self {
        Vec2Padded {
            x: v.x,
            y: v.y,
            padding0: 0.0,
            padding1: 0.0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub padding: f32,
}

impl From<glam::Vec3> for Vec3 {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Vec3 {
            x: v.x,
            y: v.y,
            z: v.z,
            padding: 0.0,
        }
    }
}

impl From<glam::Vec3A> for Vec3 {
    #[inline]
    fn from(v: glam::Vec3A) -> Self {
        Vec3 {
            x: v.x,
            y: v.y,
            z: v.z,
            padding: 0.0,
        }
    }
}

#[repr(C, align(4))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec3Unpadded {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<glam::Vec3> for Vec3Unpadded {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Vec3Unpadded {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<glam::Vec3A> for Vec3Unpadded {
    #[inline]
    fn from(v: glam::Vec3A) -> Self {
        Vec3Unpadded {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl From<glam::Vec4> for Vec4 {
    #[inline]
    fn from(v: glam::Vec4) -> Self {
        Vec4 {
            x: v.x,
            y: v.y,
            z: v.z,
            w: v.w,
        }
    }
}

impl From<crate::Rgba> for Vec4 {
    #[inline]
    fn from(c: crate::Rgba) -> Self {
        Vec4 {
            x: c.r(),
            y: c.g(),
            z: c.b(),
            w: c.a(),
        }
    }
}

// --- UVec ---

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct UVec3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub padding: u32,
}

impl From<glam::UVec3> for UVec3 {
    #[inline]
    fn from(v: glam::UVec3) -> Self {
        UVec3 {
            x: v.x,
            y: v.y,
            z: v.z,
            padding: 0,
        }
    }
}

#[repr(C, align(4))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct UVec3Unpadded {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl From<glam::UVec3> for UVec3Unpadded {
    #[inline]
    fn from(v: glam::UVec3) -> Self {
        UVec3Unpadded {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

// --- Mat ---

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Mat4 {
    c0: Vec4,
    c1: Vec4,
    c2: Vec4,
    c3: Vec4,
}

impl From<glam::Mat4> for Mat4 {
    #[inline]
    fn from(m: glam::Mat4) -> Self {
        Mat4 {
            c0: m.x_axis.into(),
            c1: m.y_axis.into(),
            c2: m.z_axis.into(),
            c3: m.w_axis.into(),
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Mat4x3 {
    c0: Vec3,
    c1: Vec3,
    c2: Vec3,
    c3: Vec3,
}

impl From<glam::Affine3A> for Mat4x3 {
    #[inline]
    fn from(m: glam::Affine3A) -> Self {
        Mat4x3 {
            c0: m.matrix3.x_axis.into(),
            c1: m.matrix3.y_axis.into(),
            c2: m.matrix3.z_axis.into(),
            c3: m.translation.into(),
        }
    }
}
