use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

use super::{mat::Mat3x3, Quaternion, Vec2D, Vec3D};

// TODO: More docs.

/// 3D scaling factor.
///
/// ```
/// use re_log_types::component_types::{Scale3D, Vec3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Scale3D::data_type(),
///     DataType::Union(vec![
///         Field::new("Unit", DataType::Boolean, false),
///         Field::new("ThreeD", Vec3D::data_type(), false),
///         Field::new("Uniform", DataType::Float32, false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Scale3D {
    /// Unit scale, meaning no scaling.
    #[default]
    Unit,

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
            Scale3D::Unit => glam::Vec3::ONE,
            Scale3D::ThreeD(v) => v.into(),
            Scale3D::Uniform(v) => glam::Vec3::splat(v),
        }
    }
}

// TODO:
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Angle {
    Radians(f32),
    Degrees(f32),
}

impl Angle {
    #[inline]
    pub fn radians(&self) -> f32 {
        match self {
            Self::Radians(v) => *v,
            Self::Degrees(v) => v.to_radians(),
        }
    }

    #[inline]
    pub fn degrees(&self) -> f32 {
        match self {
            Self::Radians(v) => v.to_degrees(),
            Self::Degrees(v) => *v,
        }
    }
}

// TODO:
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AxisAngleRotation {
    pub axis: Vec3D,
    pub angle: Angle,
}

impl AxisAngleRotation {
    #[inline]
    pub fn new<V: Into<Vec3D>>(axis: V, angle: Angle) -> Self {
        Self {
            axis: axis.into(),
            angle,
        }
    }
}

#[cfg(feature = "glam")]
impl From<AxisAngleRotation> for glam::Quat {
    #[inline]
    fn from(val: AxisAngleRotation) -> Self {
        glam::Quat::from_axis_angle(val.axis.into(), val.angle.radians())
    }
}

/// 3D rotation.
///
/// ```
/// use re_log_types::component_types::{Quaternion, Rotation3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Rotation3D::data_type(),
///     DataType::Union(vec![
///         Field::new("Identity", DataType::Boolean, false),
///         Field::new("Quaternion", Quaternion::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Rotation3D {
    /// No rotation.
    #[default]
    Identity,

    /// Rotation defined by a quaternion.
    Quaternion(Quaternion),

    /// Rotation defined with an axis and an angle.
    AxisAngle(AxisAngleRotation),
}

impl From<Quaternion> for Rotation3D {
    #[inline]
    fn from(q: Quaternion) -> Self {
        Self::Quaternion(q)
    }
}

impl From<AxisAngleRotation> for Rotation3D {
    #[inline]
    fn from(r: AxisAngleRotation) -> Self {
        Self::AxisAngle(r)
    }
}

#[cfg(feature = "glam")]
impl From<Rotation3D> for glam::Quat {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        match val {
            Rotation3D::Identity => glam::Quat::IDENTITY,
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

/// Representation of a affine transform via a 3x3 translation matrix paired with a translation.
///
/// First applies the matrix, then the translation.
///
/// ```
/// use re_log_types::component_types::{TranslationMatrix3x3, Vec3D, Mat3x3};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     TranslationMatrix3x3::data_type(),
///     DataType::Struct(vec![
///         Field::new("translation", Vec3D::data_type(), false),
///         Field::new("matrix", Mat3x3::data_type(), false)
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TranslationMatrix3x3 {
    /// 3D translation, applied after the matrix.
    pub translation: Vec3D,

    /// 3x3 matrix for scale, rotation & skew.
    pub matrix: Mat3x3,
}

impl TranslationMatrix3x3 {
    pub const IDENTITY: TranslationMatrix3x3 = TranslationMatrix3x3 {
        translation: Vec3D::ZERO,
        matrix: Mat3x3::IDENTITY,
    };
}

/// Representation of an affine transform via separate translation, rotation & scale.
///
/// ```
/// use re_log_types::component_types::{TranslationRotationScale, Rotation3D, Scale3D, Vec3D};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     TranslationRotationScale::data_type(),
///     DataType::Struct(vec![
///         Field::new("translation", Vec3D::data_type(), false),
///         Field::new("rotation", Rotation3D::data_type(), false),
///         Field::new("scale", Scale3D::data_type(), false)
///     ]),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TranslationRotationScale3D {
    /// 3D translation vector, applied last.
    pub translation: Vec3D,

    /// 3D rotation, applied second.
    pub rotation: Rotation3D,

    /// 3D scale, applied first.
    pub scale: Scale3D,
}

impl TranslationRotationScale3D {
    pub const IDENTITY: TranslationRotationScale3D = TranslationRotationScale3D {
        translation: Vec3D::ZERO,
        rotation: Rotation3D::Identity,
        scale: Scale3D::Unit,
    };
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
            translation: v,
            ..Default::default()
        }
    }
}

impl From<Rotation3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Rotation3D) -> Self {
        Self {
            rotation: v,
            ..Default::default()
        }
    }
}

impl From<Scale3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Scale3D) -> Self {
        Self {
            scale: v,
            ..Default::default()
        }
    }
}

/// Representation of a 3D affine transform.
///
/// ```
/// use re_log_types::component_types::{Affine3DRepresentation, TranslationMatrix3x3, TranslationRotationScale};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Affine3DRepresentation::data_type(),
///     DataType::Union(vec![
///         Field::new("TranslationMatrix3x3", TranslationMatrix3x3::data_type(), false),
///         Field::new("TranslationRotationScale", TranslationRotationScale::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Affine3D {
    TranslationMatrix3x3(TranslationMatrix3x3),
    TranslationRotationScale(TranslationRotationScale3D),
    // TODO: Raw 4x4 matrix.
}

impl Affine3D {
    pub const IDENTITY: Affine3D = Affine3D::TranslationMatrix3x3(TranslationMatrix3x3::IDENTITY);

    /// Affine transform from a translation only.
    #[inline]
    pub fn from_translation<T: Into<Vec3D>>(translation: T) -> Self {
        Self::TranslationMatrix3x3(TranslationMatrix3x3 {
            translation: translation.into(),
            matrix: Mat3x3::IDENTITY,
        })
    }

    /// Affine transform from a rotation only.
    #[inline]
    pub fn from_rotation<R: Into<Rotation3D>>(rotation: R) -> Self {
        Self::TranslationRotationScale(TranslationRotationScale3D {
            rotation: rotation.into(),
            ..Default::default()
        })
    }

    /// Affine transform from a scale only.
    #[inline]
    pub fn from_scale<S: Into<Scale3D>>(scale: S) -> Self {
        Self::TranslationRotationScale(TranslationRotationScale3D {
            scale: scale.into(),
            ..Default::default()
        })
    }

    /// Affine transform from a translation, applied after a rotation.
    #[inline]
    pub fn from_translation_rotation<T: Into<Vec3D>, R: Into<Rotation3D>>(
        translation: T,
        rotation: R,
    ) -> Self {
        Self::TranslationRotationScale(TranslationRotationScale3D {
            translation: translation.into(),
            rotation: rotation.into(),
            scale: Scale3D::Unit,
        })
    }

    /// Affine transform from a translation, applied after a linear transformation via a 3x3 matrix.
    #[inline]
    pub fn from_translation_matrix<T: Into<Vec3D>, M: Into<Mat3x3>>(
        translation: T,
        matrix: M,
    ) -> Self {
        Self::TranslationMatrix3x3(TranslationMatrix3x3 {
            translation: translation.into(),
            matrix: matrix.into(),
        })
    }

    /// Affine transform from a translation, applied after a rotation, applied after a scale.
    #[inline]
    pub fn from_translation_rotation_scale<
        T: Into<Vec3D>,
        R: Into<Rotation3D>,
        S: Into<Scale3D>,
    >(
        translation: T,
        rotation: R,
        scale: S,
    ) -> Self {
        Self::TranslationRotationScale(TranslationRotationScale3D {
            translation: translation.into(),
            rotation: rotation.into(),
            scale: scale.into(),
        })
    }

    #[inline]
    pub fn parent_from_child(self) -> DirectedAffine3D {
        DirectedAffine3D::ParentFromChild(self)
    }

    #[inline]
    pub fn child_from_parent(self) -> DirectedAffine3D {
        DirectedAffine3D::ChildFromParent(self)
    }
}

impl Default for Affine3D {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<TranslationMatrix3x3> for Affine3D {
    #[inline]
    fn from(v: TranslationMatrix3x3) -> Self {
        Self::TranslationMatrix3x3(v)
    }
}

impl From<TranslationRotationScale3D> for Affine3D {
    #[inline]
    fn from(v: TranslationRotationScale3D) -> Self {
        Self::TranslationRotationScale(v)
    }
}

#[cfg(feature = "glam")]
impl From<Affine3D> for glam::Affine3A {
    fn from(value: Affine3D) -> Self {
        match value {
            Affine3D::TranslationMatrix3x3(TranslationMatrix3x3 {
                translation,
                matrix,
            }) => glam::Affine3A::from_mat3_translation(matrix.into(), translation.into()),

            Affine3D::TranslationRotationScale(TranslationRotationScale3D {
                translation,
                rotation,
                scale,
            }) => glam::Affine3A::from_scale_rotation_translation(
                scale.into(),
                rotation.into(),
                translation.into(),
            ),
        }
    }
}

/// An affine transform between two 3D spaces, represented in a given direction.
///
/// TODO: doctest
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum DirectedAffine3D {
    /// The transform maps from the parent space to the child space.
    ChildFromParent(Affine3D),

    /// The transform maps from the child space to the parent space.
    ParentFromChild(Affine3D),
}

impl DirectedAffine3D {
    /// Identity transform, i.e. parent & child are in the same space.
    ///
    /// Uses [`DirectedAffine3D::ParentFromChild`] since this is more common internally,
    /// as we usually transform everything into a parent space.
    pub const IDENTITY: DirectedAffine3D = DirectedAffine3D::ParentFromChild(Affine3D::IDENTITY);
}

impl Default for DirectedAffine3D {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(feature = "glam")]
impl DirectedAffine3D {
    #[inline]
    pub fn parent_from_child_transform(self) -> glam::Affine3A {
        match self {
            DirectedAffine3D::ChildFromParent(transform) => {
                let transform: glam::Affine3A = transform.into();
                transform.inverse()
            }
            DirectedAffine3D::ParentFromChild(transform) => transform.into(),
        }
    }

    #[inline]
    pub fn child_from_parent_transform(self) -> glam::Affine3A {
        match self {
            DirectedAffine3D::ChildFromParent(transform) => transform.into(),
            DirectedAffine3D::ParentFromChild(transform) => {
                let transform: glam::Affine3A = transform.into();
                transform.inverse()
            }
        }
    }
}

/// Camera perspective projection (a.k.a. intrinsics).
///
///
/// ```
/// use re_log_types::component_types::Pinhole;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Pinhole::data_type(),
///     DataType::Struct(vec![
///         Field::new(
///             "image_from_cam",
///             DataType::FixedSizeList(
///                 Box::new(Field::new("item", DataType::Float32, false)),
///                 9
///             ),
///             false,
///         ),
///         Field::new(
///             "resolution",
///             DataType::FixedSizeList(
///                 Box::new(Field::new("item", DataType::Float32, false)),
///                 2
///             ),
///             true,
///         ),
///     ]),
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Pinhole {
    /// Column-major projection matrix.
    ///
    /// Child from parent.
    /// Image coordinates from camera view coordinates.
    ///
    /// Example:
    /// ```text
    /// [[1496.1, 0.0,    0.0], // col 0
    ///  [0.0,    1496.1, 0.0], // col 1
    ///  [980.5,  744.5,  1.0]] // col 2
    /// ```
    pub image_from_cam: Mat3x3,

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// [`Self::image_from_cam`] project onto the space spanned by `(0,0)` and `resolution - 1`.
    pub resolution: Option<Vec2D>,
}

impl Pinhole {
    /// Field of View on the Y axis, i.e. the angle between top and bottom (in radians).
    #[inline]
    pub fn fov_y(&self) -> Option<f32> {
        self.resolution
            .map(|resolution| 2.0 * (0.5 * resolution[1] / self.image_from_cam[1][1]).atan())
    }

    /// X & Y focal length in pixels.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[inline]
    pub fn focal_length_in_pixels(&self) -> Vec2D {
        [self.image_from_cam[0][0], self.image_from_cam[1][1]].into()
    }

    /// Focal length.
    #[inline]
    pub fn focal_length(&self) -> Option<f32> {
        // Use only the first element of the focal length vector, as we don't support non-square pixels.
        self.resolution.map(|r| self.image_from_cam[0][0] / r[0])
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn principal_point(&self) -> glam::Vec2 {
        glam::vec2(self.image_from_cam[2][0], self.image_from_cam[2][1])
    }

    #[inline]
    #[cfg(feature = "glam")]
    pub fn resolution(&self) -> Option<glam::Vec2> {
        self.resolution.map(|r| r.into())
    }

    #[inline]
    pub fn aspect_ratio(&self) -> Option<f32> {
        self.resolution.map(|r| r[0] / r[1])
    }
}

// ----------------------------------------------------------------------------

/// A 3D transform between two spaces.
///
/// ```
/// use re_log_types::component_types::{Transform3D, DirectedAffine3D, Pinhole};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Transform3D::data_type(),
///     DataType::Union(
///        vec![
///            Field::new("Unknown", DataType::Boolean, false),
///            Field::new("Affine3D", DirectedAffine3D::data_type(), false),
///            Field::new("Pinhole", Pinhole::data_type(), false),
///        ],
///        None,
///        UnionMode::Dense
///     )
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Transform3D {
    /// We don't know the transform, but it is likely/potentially non-identity.
    /// Maybe the user intend to set the transform later.
    Unknown,

    /// Affine transform between two 3D spaces.
    ///
    /// For example: the parent is a 3D world space, the child a camera space.
    Affine3D(DirectedAffine3D),

    /// The parent is some local camera space, the child an image space.
    Pinhole(Pinhole),
}

impl Component for Transform3D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.transform3d".into()
    }
}

impl From<DirectedAffine3D> for Transform3D {
    #[inline]
    fn from(affine: DirectedAffine3D) -> Self {
        Self::Affine3D(affine)
    }
}

impl From<Pinhole> for Transform3D {
    #[inline]
    fn from(pinhole: Pinhole) -> Self {
        Self::Pinhole(pinhole)
    }
}

#[test]
fn test_transform_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let transforms_in = vec![
        Transform3D::Pinhole(Pinhole {
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
            resolution: None,
        }),
        Transform3D::Affine3D(DirectedAffine3D::ChildFromParent(
            TranslationMatrix3x3 {
                translation: [10.0, 11.0, 12.0].into(),
                matrix: [[13.0, 14.0, 15.0], [16.0, 17.0, 18.0], [19.0, 20.0, 21.0]].into(),
            }
            .into(),
        )),
        Transform3D::Affine3D(DirectedAffine3D::ChildFromParent(
            TranslationRotationScale3D {
                translation: [10.0, 11.0, 12.0].into(),
                rotation: Quaternion::new(13.0, 14.0, 15.0, 16.0).into(),
                scale: [17.0, 18.0, 19.0].into(),
            }
            .into(),
        )),
        Transform3D::Pinhole(Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]].into(),
            resolution: Some([123.0, 456.0].into()),
        }),
    ];
    let array: Box<dyn Array> = transforms_in.try_into_arrow().unwrap();
    let transforms_out: Vec<Transform3D> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(transforms_in, transforms_out);
}
