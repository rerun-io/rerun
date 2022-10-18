#[repr(C)]
#[repr(align(8))]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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

#[repr(C)]
#[repr(align(16))]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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

#[repr(C)]
#[repr(align(16))]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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

#[repr(C)]
#[repr(align(16))]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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
