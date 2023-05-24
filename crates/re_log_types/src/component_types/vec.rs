use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use super::FixedSizeArrayField;
use crate::Component;

/// Number of decimals shown for all vector display methods.
const DISPLAY_PRECISION: usize = 3;

// --- Vec2D ---

/// A vector in 2D space.
///
/// ```
/// # use re_log_types::component_types::Vec2D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     Vec2D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         2
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Vec2D(#[arrow_field(type = "FixedSizeArrayField<f32,2>")] pub [f32; 2]);

impl Vec2D {
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }
}

impl From<[f32; 2]> for Vec2D {
    #[inline]
    fn from(v: [f32; 2]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec2D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl Component for Vec2D {
    fn name() -> crate::ComponentName {
        "rerun.vec2d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec2D> for glam::Vec2 {
    fn from(v: Vec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Vec2D {
    fn from(v: glam::Vec2) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for Vec2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            prec = DISPLAY_PRECISION,
        )
    }
}

// --- Vec3D ---

/// A vector in 3D space.
///
/// ```
/// use re_log_types::component_types::Vec3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Vec3D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         3
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Vec3D(#[arrow_field(type = "FixedSizeArrayField<f32,3>")] pub [f32; 3]);

impl Vec3D {
    pub const ZERO: Vec3D = Vec3D([0.0; 3]);

    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self::from([x, y, z])
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }
}

impl From<[f32; 3]> for Vec3D {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec3D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl Component for Vec3D {
    fn name() -> crate::ComponentName {
        "rerun.vec3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec3D> for glam::Vec3 {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for Vec3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            self.z(),
            prec = DISPLAY_PRECISION,
        )
    }
}

// --- Vec4D ---

/// A vector in 4D space.
///
/// ```
/// # use re_log_types::component_types::Vec4D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     Vec4D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         4
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Vec4D(#[arrow_field(type = "FixedSizeArrayField<f32,4>")] pub [f32; 4]);

impl Vec4D {
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn w(&self) -> f32 {
        self.0[3]
    }
}

impl std::fmt::Display for Vec4D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}, {:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            self.z(),
            self.w(),
            prec = DISPLAY_PRECISION
        )
    }
}

#[test]
fn test_vec4d() {
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};
    let data = [Vec4D([0.0, 1.0, 2.0, 3.0]), Vec4D([0.1, 1.1, 2.1, 3.1])];
    let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
    let ret: Vec<Vec4D> = array.try_into_collection().unwrap();
    assert_eq!(&data, ret.as_slice());
}
