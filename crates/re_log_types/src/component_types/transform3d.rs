use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

use super::{mat::Mat3x3, Quaternion, Vec3D};

/// 3D scaling factor, part of a transform representation.
///
/// ```
/// use re_log_types::component_types::{Scale3D, Vec3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Scale3D::data_type(),
///     DataType::Union(vec![
///         Field::new("ThreeD", Vec3D::data_type(), false),
///         Field::new("Uniform", DataType::Float32, false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Scale3D {
    /// Individual scaling factors for each axis, distorting the original object.
    ThreeD(Vec3D),

    /// Uniform scaling factor along all axis.
    Uniform(f32),
}

impl From<Vec3D> for Scale3D {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self::ThreeD(v)
    }
}

impl From<f32> for Scale3D {
    #[inline]
    fn from(v: f32) -> Self {
        Self::Uniform(v)
    }
}

impl From<[f32; 3]> for Scale3D {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self::ThreeD(v.into())
    }
}

#[cfg(feature = "glam")]
impl From<Scale3D> for glam::Vec3 {
    #[inline]
    fn from(val: Scale3D) -> Self {
        match val {
            Scale3D::ThreeD(v) => v.into(),
            Scale3D::Uniform(v) => glam::Vec3::splat(v),
        }
    }
}

/// Angle in either radians or degrees.
///
/// ```
/// use re_log_types::component_types::Angle;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Angle::data_type(),
///     DataType::Union(vec![
///         Field::new("Radians", DataType::Float32, false),
///         Field::new("Degrees", DataType::Float32, false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Angle {
    Radians(f32),
    Degrees(f32),
}

impl Angle {
    /// Angle in radians independent of the underlying representation.
    #[inline]
    pub fn radians(&self) -> f32 {
        match self {
            Self::Radians(v) => *v,
            Self::Degrees(v) => v.to_radians(),
        }
    }

    /// Angle in degrees independent of the underlying representation.
    #[inline]
    pub fn degrees(&self) -> f32 {
        match self {
            Self::Radians(v) => v.to_degrees(),
            Self::Degrees(v) => *v,
        }
    }
}

/// 3D rotation represented by a rotation around a given axis.
///
/// ```
/// use re_log_types::component_types::{RotationAxisAngle, Angle, Vec3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     RotationAxisAngle::data_type(),
///     DataType::Struct(vec![
///         Field::new("axis", Vec3D::data_type(), false),
///         Field::new("angle", Angle::data_type(), false),
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RotationAxisAngle {
    /// Axis to rotate around.
    ///
    /// This is not required to be normalized.
    /// If normalization fails (typically because the vector is length zero), the rotation is silently ignored.
    pub axis: Vec3D,

    /// How much to rotate around the axis.
    pub angle: Angle,
}

impl RotationAxisAngle {
    #[inline]
    pub fn new<V: Into<Vec3D>>(axis: V, angle: Angle) -> Self {
        Self {
            axis: axis.into(),
            angle,
        }
    }
}

#[cfg(feature = "glam")]
impl From<RotationAxisAngle> for glam::Quat {
    #[inline]
    fn from(val: RotationAxisAngle) -> Self {
        let axis: glam::Vec3 = val.axis.into();
        axis.try_normalize()
            .map(|axis| glam::Quat::from_axis_angle(axis, val.angle.radians()))
            .unwrap_or_default()
    }
}

/// A 3D rotation.
///
/// ```
/// use re_log_types::component_types::{Quaternion, Rotation3D, RotationAxisAngle};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Rotation3D::data_type(),
///     DataType::Union(vec![
///         Field::new("Quaternion", Quaternion::data_type(), false),
///         Field::new("AxisAngle", RotationAxisAngle::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Rotation3D {
    /// Rotation defined by a quaternion.
    Quaternion(Quaternion),

    /// Rotation defined with an axis and an angle.
    AxisAngle(RotationAxisAngle),
}

impl From<Quaternion> for Rotation3D {
    #[inline]
    fn from(q: Quaternion) -> Self {
        Self::Quaternion(q)
    }
}

impl From<RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(r: RotationAxisAngle) -> Self {
        Self::AxisAngle(r)
    }
}

#[cfg(feature = "glam")]
impl From<Rotation3D> for glam::Quat {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        match val {
            Rotation3D::Quaternion(v) => v.into(),
            Rotation3D::AxisAngle(a) => a.into(),
        }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Quat> for Rotation3D {
    #[inline]
    fn from(val: glam::Quat) -> Self {
        Rotation3D::Quaternion(val.into())
    }
}

/// Representation of a affine transform via a 3x3 affine matrix paired with a translation.
///
/// First applies the matrix, then the translation.
///
/// ```
/// use re_log_types::component_types::{TranslationAndMat3, Vec3D, Mat3x3};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     TranslationAndMat3::data_type(),
///     DataType::Struct(vec![
///         Field::new("translation", Vec3D::data_type(), true),
///         Field::new("matrix", Mat3x3::data_type(), false)
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TranslationAndMat3 {
    /// 3D translation, applied after the matrix.
    pub translation: Option<Vec3D>,

    /// 3x3 matrix for scale, rotation & shear.
    pub matrix: Mat3x3,
}

impl TranslationAndMat3 {
    pub const IDENTITY: TranslationAndMat3 = TranslationAndMat3 {
        translation: None,
        matrix: Mat3x3::IDENTITY,
    };

    /// Create a new `TranslationAndMat3`.
    #[inline]
    pub fn new<T: Into<Vec3D>, M: Into<Mat3x3>>(translation: T, matrix: M) -> Self {
        Self {
            translation: Some(translation.into()),
            matrix: matrix.into(),
        }
    }
}

/// Representation of an affine transform via separate translation, rotation & scale.
///
/// ```
/// use re_log_types::component_types::{TranslationRotationScale3D, Rotation3D, Scale3D, Vec3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     TranslationRotationScale3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("translation", Vec3D::data_type(), true),
///         Field::new("rotation", Rotation3D::data_type(), true),
///         Field::new("scale", Scale3D::data_type(), true)
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TranslationRotationScale3D {
    /// 3D translation vector, applied last.
    pub translation: Option<Vec3D>,

    /// 3D rotation, applied second.
    pub rotation: Option<Rotation3D>,

    /// 3D scale, applied first.
    pub scale: Option<Scale3D>,
}

impl TranslationRotationScale3D {
    pub const IDENTITY: TranslationRotationScale3D = TranslationRotationScale3D {
        translation: None,
        rotation: None,
        scale: None,
    };

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn rigid<T: Into<Vec3D>, R: Into<Rotation3D>>(translation: T, rotation: R) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: None,
        }
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn affine<T: Into<Vec3D>, R: Into<Rotation3D>, S: Into<Scale3D>>(
        translation: T,
        rotation: R,
        scale: S,
    ) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: Some(scale.into()),
        }
    }
}

impl Default for TranslationRotationScale3D {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<Vec3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self {
            translation: Some(v),
            ..Default::default()
        }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for TranslationRotationScale3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self {
            translation: Some(v.into()),
            ..Default::default()
        }
    }
}

impl From<Rotation3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Rotation3D) -> Self {
        Self {
            rotation: Some(v),
            ..Default::default()
        }
    }
}

impl From<Scale3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Scale3D) -> Self {
        Self {
            scale: Some(v),
            ..Default::default()
        }
    }
}

/// Representation of a 3D affine transform.
///
/// Rarely used directly, prefer using the underlying representation classes and pass them directly to
/// [`Transform3D::new`] or [`Transform3D::from_parent`].
///
/// ```
/// use re_log_types::component_types::{Transform3DRepr, TranslationAndMat3, TranslationRotationScale3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Transform3DRepr::data_type(),
///     DataType::Union(vec![
///         Field::new("TranslationAndMat3", TranslationAndMat3::data_type(), false),
///         Field::new("TranslationRotationScale", TranslationRotationScale3D::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Transform3DRepr {
    TranslationAndMat3(TranslationAndMat3),
    TranslationRotationScale(TranslationRotationScale3D),
    // TODO(andreas): Raw 4x4 matrix.
}

impl Transform3DRepr {
    pub const IDENTITY: Transform3DRepr =
        Transform3DRepr::TranslationRotationScale(TranslationRotationScale3D::IDENTITY);
}

impl Default for Transform3DRepr {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<TranslationAndMat3> for Transform3DRepr {
    #[inline]
    fn from(v: TranslationAndMat3) -> Self {
        Self::TranslationAndMat3(v)
    }
}

impl From<TranslationRotationScale3D> for Transform3DRepr {
    #[inline]
    fn from(v: TranslationRotationScale3D) -> Self {
        Self::TranslationRotationScale(v)
    }
}

impl From<Vec3D> for Transform3DRepr {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self::TranslationRotationScale(v.into())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Transform3DRepr {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self::TranslationRotationScale(v.into())
    }
}

impl From<RotationAxisAngle> for Transform3DRepr {
    #[inline]
    fn from(v: RotationAxisAngle) -> Self {
        let rotation = Rotation3D::from(v);
        Self::TranslationRotationScale(rotation.into())
    }
}

#[cfg(feature = "glam")]
impl From<Transform3DRepr> for glam::Affine3A {
    fn from(value: Transform3DRepr) -> Self {
        match value {
            Transform3DRepr::TranslationAndMat3(TranslationAndMat3 {
                translation,
                matrix,
            }) => glam::Affine3A::from_mat3_translation(
                matrix.into(),
                translation.map_or(glam::Vec3::ZERO, |v| v.into()),
            ),

            Transform3DRepr::TranslationRotationScale(TranslationRotationScale3D {
                translation,
                rotation,
                scale,
            }) => glam::Affine3A::from_scale_rotation_translation(
                scale.map_or(glam::Vec3::ONE, |s| s.into()),
                rotation.map_or(glam::Quat::IDENTITY, |q| q.into()),
                translation.map_or(glam::Vec3::ZERO, |v| v.into()),
            ),
        }
    }
}

/// An affine transform between two 3D spaces, represented in a given direction.
///
/// This component is a "mono-component". See [the crate level docs](crate) for details.
///
/// ```
/// use re_log_types::component_types::{Transform3DRepr, Transform3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Transform3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("transform", Transform3DRepr::data_type(), false),
///         Field::new("from_parent", DataType::Boolean, false),
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Transform3D {
    /// Representation of the transform.
    pub transform: Transform3DRepr,

    /// If true, the transform maps from the parent space to the space where the transform was logged.
    /// Otherwise, the transform maps from the space to its parent.
    pub from_parent: bool,
}

impl Transform3D {
    /// Identity transform, i.e. parent & child are in the same space.
    pub const IDENTITY: Transform3D = Transform3D {
        transform: Transform3DRepr::IDENTITY,
        from_parent: false,
    };

    /// Creates a new transform with a given representation, transforming from the parent space into the child space.
    pub fn new<T: Into<Transform3DRepr>>(representation: T) -> Self {
        Self {
            transform: representation.into(),
            from_parent: false,
        }
    }

    /// Creates a new transform with a given representation, transforming from the child space into the parent space.
    pub fn from_parent<T: Into<Transform3DRepr>>(representation: T) -> Self {
        Self {
            transform: representation.into(),
            from_parent: true,
        }
    }
}

#[cfg(feature = "glam")]
impl Transform3D {
    #[inline]
    pub fn to_parent_from_child_transform(&self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.transform.into();
        if self.from_parent {
            transform.inverse()
        } else {
            transform
        }
    }

    #[inline]
    pub fn to_child_from_parent_transform(self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.transform.into();
        if self.from_parent {
            transform
        } else {
            transform.inverse()
        }
    }
}

impl Component for Transform3D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.transform3d".into()
    }
}

#[test]
fn test_transform_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let transforms_in = vec![
        Transform3D::from_parent(TranslationAndMat3 {
            translation: Some([10.0, 11.0, 12.0].into()),
            matrix: [[13.0, 14.0, 15.0], [16.0, 17.0, 18.0], [19.0, 20.0, 21.0]].into(),
        }),
        Transform3D::new(TranslationRotationScale3D {
            translation: Some([10.0, 11.0, 12.0].into()),
            rotation: Some(Quaternion::new(13.0, 14.0, 15.0, 16.0).into()),
            scale: Some([17.0, 18.0, 19.0].into()),
        }),
    ];
    let array: Box<dyn Array> = transforms_in.try_into_arrow().unwrap();
    let transforms_out: Vec<Transform3D> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(transforms_in, transforms_out);
}
