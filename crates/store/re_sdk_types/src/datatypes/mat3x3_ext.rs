use super::Mat3x3;
use crate::datatypes::Vec3D;

impl Mat3x3 {
    /// The identity matrix.
    ///
    /// The multiplicative identity, representing no transform.
    #[rustfmt::skip]
    pub const IDENTITY: Self = Self([
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
    ]);

    /// Returns the matrix column for the given `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than 2.
    #[inline]
    pub fn col(&self, index: usize) -> Vec3D {
        match index {
            0 => [self.0[0], self.0[1], self.0[2]].into(),
            1 => [self.0[3], self.0[4], self.0[5]].into(),
            2 => [self.0[6], self.0[7], self.0[8]].into(),
            _ => panic!("index out of bounds"),
        }
    }

    /// Get a specific element.
    // NOTE: row-col is the normal index order for matrices in mathematics.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f32 {
        assert!(
            row < 3 && col < 3,
            "Mat3x3 index out of bounds (row: {row}, col: {col})"
        );
        self.0[row + col * 3]
    }

    /// Set a specific element.
    // NOTE: row-col is the normal index order for matrices in mathematics.
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, value: f32) {
        assert!(
            row < 3 && col < 3,
            "Mat3x3 index out of bounds (row: {row}, col: {col})"
        );
        self.0[row + col * 3] = value;
    }
}

impl<Idx> std::ops::Index<Idx> for Mat3x3
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    /// Column-major order matrix coefficients.
    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl From<[Vec3D; 3]> for Mat3x3 {
    #[inline]
    #[rustfmt::skip]
    fn from(v: [Vec3D; 3]) -> Self {
        Self([
            v[0].x(), v[0].y(), v[0].z(),
            v[1].x(), v[1].y(), v[1].z(),
            v[2].x(), v[2].y(), v[2].z(),
        ])
    }
}

impl From<Mat3x3> for [Vec3D; 3] {
    fn from(val: Mat3x3) -> Self {
        [
            [val.0[0], val.0[1], val.0[2]].into(), //
            [val.0[3], val.0[4], val.0[5]].into(), //
            [val.0[6], val.0[7], val.0[8]].into(), //
        ]
    }
}

impl From<[[f32; 3]; 3]> for Mat3x3 {
    #[inline]
    fn from(v: [[f32; 3]; 3]) -> Self {
        Self::from([Vec3D(v[0]), Vec3D(v[1]), Vec3D(v[2])])
    }
}

#[cfg(feature = "glam")]
impl From<Mat3x3> for glam::Mat3 {
    #[inline]
    fn from(v: Mat3x3) -> Self {
        Self::from_cols_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Mat3> for Mat3x3 {
    #[inline]
    fn from(v: glam::Mat3) -> Self {
        Self::from(v.to_cols_array_2d())
    }
}

#[cfg(feature = "mint")]
impl From<Mat3x3> for mint::ColumnMatrix3<f32> {
    #[inline]
    fn from(v: Mat3x3) -> Self {
        v.0.into()
    }
}

#[cfg(feature = "mint")]
impl From<mint::ColumnMatrix3<f32>> for Mat3x3 {
    #[inline]
    fn from(v: mint::ColumnMatrix3<f32>) -> Self {
        std::convert::From::<[[f32; 3]; 3]>::from([v.x.into(), v.y.into(), v.z.into()])
    }
}

impl Default for Mat3x3 {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}
