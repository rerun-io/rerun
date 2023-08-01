use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use super::FixedSizeArrayField;

/// Number of decimals shown for all vector display methods.
const DISPLAY_PRECISION: usize = 3;

// --- Vec2D ---

/// A vector in 2D space.
///
/// ```
/// # use re_components::LegacyVec2D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     LegacyVec2D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         2
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct LegacyVec2D(#[arrow_field(type = "FixedSizeArrayField<f32,2>")] pub [f32; 2]);

impl LegacyVec2D {
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }
}

impl From<[f32; 2]> for LegacyVec2D {
    #[inline]
    fn from(v: [f32; 2]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for LegacyVec2D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl re_log_types::LegacyComponent for LegacyVec2D {
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.vec2d".into()
    }
}

#[cfg(feature = "glam")]
impl From<LegacyVec2D> for glam::Vec2 {
    fn from(v: LegacyVec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for LegacyVec2D {
    fn from(v: glam::Vec2) -> Self {
        Self(v.to_array())
    }
}

impl From<re_types::datatypes::Vec2D> for LegacyVec2D {
    fn from(value: re_types::datatypes::Vec2D) -> Self {
        Self(value.0)
    }
}

impl std::fmt::Display for LegacyVec2D {
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

re_log_types::component_legacy_shim!(LegacyVec2D);

// --- Vec3D ---

/// A vector in 3D space.
///
/// ```
/// use re_components::LegacyVec3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     LegacyVec3D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         3
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct LegacyVec3D(#[arrow_field(type = "FixedSizeArrayField<f32,3>")] pub [f32; 3]);

impl LegacyVec3D {
    pub const ZERO: LegacyVec3D = LegacyVec3D([0.0; 3]);

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

impl From<[f32; 3]> for LegacyVec3D {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for LegacyVec3D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl re_log_types::LegacyComponent for LegacyVec3D {
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.vec3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<LegacyVec3D> for glam::Vec3 {
    #[inline]
    fn from(v: LegacyVec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for LegacyVec3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self(v.to_array())
    }
}

impl From<re_types::datatypes::Vec3D> for LegacyVec3D {
    fn from(value: re_types::datatypes::Vec3D) -> Self {
        Self(value.0)
    }
}

impl std::fmt::Display for LegacyVec3D {
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

re_log_types::component_legacy_shim!(LegacyVec3D);

// --- Vec4D ---

/// A vector in 4D space.
///
/// ```
/// # use re_components::LegacyVec4D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     LegacyVec4D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Float32, false)),
///         4
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct LegacyVec4D(#[arrow_field(type = "FixedSizeArrayField<f32,4>")] pub [f32; 4]);

impl LegacyVec4D {
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

impl std::fmt::Display for LegacyVec4D {
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

impl From<re_types::datatypes::Vec4D> for LegacyVec4D {
    fn from(value: re_types::datatypes::Vec4D) -> Self {
        Self(value.0)
    }
}

#[test]
fn test_vec4d() {
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};
    let data = [
        LegacyVec4D([0.0, 1.0, 2.0, 3.0]),
        LegacyVec4D([0.1, 1.1, 2.1, 3.1]),
    ];
    let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
    let ret: Vec<LegacyVec4D> = array.try_into_collection().unwrap();
    assert_eq!(&data, ret.as_slice());
}
