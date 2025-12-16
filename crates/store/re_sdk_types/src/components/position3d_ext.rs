use super::Position3D;
use crate::datatypes::Vec3D;

// ---

impl Position3D {
    /// The origin.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Create a new position.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3D::new(x, y, z))
    }

    /// The x coordinate, i.e. index 0
    #[inline]
    pub fn x(&self) -> f32 {
        self.0.x()
    }

    /// The y coordinate, i.e. index 1
    #[inline]
    pub fn y(&self) -> f32 {
        self.0.y()
    }

    /// The z coordinate, i.e. index 2
    #[inline]
    pub fn z(&self) -> f32 {
        self.0.z()
    }
}

#[cfg(feature = "glam")]
impl From<Position3D> for glam::Vec3 {
    #[inline]
    fn from(pt: Position3D) -> Self {
        Self::new(pt.x(), pt.y(), pt.z())
    }
}

#[cfg(feature = "mint")]
impl From<Position3D> for mint::Point3<f32> {
    #[inline]
    fn from(position: Position3D) -> Self {
        Self {
            x: position.x(),
            y: position.y(),
            z: position.z(),
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Point3<f32>> for Position3D {
    #[inline]
    fn from(position: mint::Point3<f32>) -> Self {
        Self(Vec3D([position.x, position.y, position.z]))
    }
}
