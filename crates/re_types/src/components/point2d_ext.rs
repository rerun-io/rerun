use super::Point2D;

impl Point2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for Point2D {
    #[inline]
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

impl From<[f32; 2]> for Point2D {
    #[inline]
    fn from([x, y]: [f32; 2]) -> Self {
        Self { x, y }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Point2D {
    #[inline]
    fn from(pt: glam::Vec2) -> Self {
        Self::new(pt.x, pt.y)
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x, pt.y)
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec3 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x, pt.y, 0.0)
    }
}

// impl Point2D {
//     pub const ZERO: Self = Self::new(0.0, 0.0);
//     pub const ONE: Self = Self::new(1.0, 1.0);

//     #[inline]
//     pub const fn new(x: f32, y: f32) -> Self {
//         Self(crate::datatypes::Vec2D::new(x, y))
//     }
// }

// impl std::ops::Deref for Point2D {
//     type Target = crate::datatypes::Vec2D;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl From<(f32, f32)> for Point2D {
//     #[inline]
//     fn from(xy: (f32, f32)) -> Self {
//         Self(xy.into())
//     }
// }

// impl From<[f32; 2]> for Point2D {
//     #[inline]
//     fn from(p: [f32; 2]) -> Self {
//         Self(p.into())
//     }
// }

// #[cfg(feature = "glam")]
// impl From<glam::Vec2> for Point2D {
//     #[inline]
//     fn from(pt: glam::Vec2) -> Self {
//         Self::new(pt.x, pt.y)
//     }
// }

// #[cfg(feature = "glam")]
// impl From<Point2D> for glam::Vec2 {
//     #[inline]
//     fn from(pt: Point2D) -> Self {
//         Self::new(pt.x(), pt.y())
//     }
// }

// #[cfg(feature = "glam")]
// impl From<Point2D> for glam::Vec3 {
//     #[inline]
//     fn from(pt: Point2D) -> Self {
//         Self::new(pt.x(), pt.y(), 0.0)
//     }
// }
