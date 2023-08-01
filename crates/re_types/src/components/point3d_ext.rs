use super::Point3D;

// ---

impl Point3D {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(crate::datatypes::Point3D::new(x, y, z))
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0.x
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0.y
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0.z
    }
}

impl From<(f32, f32, f32)> for Point3D {
    #[inline]
    fn from((x, y, z): (f32, f32, f32)) -> Self {
        Self::new(x, y, z)
    }
}

impl From<[f32; 3]> for Point3D {
    #[inline]
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self::new(x, y, z)
    }
}

#[cfg(feature = "glam")]
impl From<Point3D> for glam::Vec3 {
    #[inline]
    fn from(pt: Point3D) -> Self {
        Self::new(pt.x(), pt.y(), pt.z())
    }
}
