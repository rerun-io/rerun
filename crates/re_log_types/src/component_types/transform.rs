use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

use super::{mat::Mat3x3, Quaternion, Vec2D, Vec3D};

/// A proper rigid 3D transform, i.e. a rotation and a translation.
///
/// Also known as an isometric transform, or a pose.
///
/// ```
/// use re_log_types::component_types::Rigid3;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Rigid3::data_type(),
///     DataType::Struct(vec![
///         Field::new(
///             "rotation",
///             DataType::FixedSizeList(
///                 Box::new(Field::new("item", DataType::Float32, false)),
///                 4
///             ),
///             false
///         ),
///         Field::new(
///             "translation",
///             DataType::FixedSizeList(
///                 Box::new(Field::new("item", DataType::Float32, false)),
///                 3
///             ),
///             false
///         )
///     ]),
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Rigid3 {
    /// How is the child rotated?
    ///
    /// This transforms to parent-space from child-space.
    pub rotation: Quaternion,

    /// Translation to parent from child.
    ///
    /// You can also think of this as the position of the child.
    pub translation: Vec3D,
}

#[cfg(feature = "glam")]
impl Rigid3 {
    pub const IDENTITY: Rigid3 = Rigid3 {
        rotation: Quaternion {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        translation: Vec3D([0.0, 0.0, 0.0]),
    };

    #[inline]
    pub fn new_parent_from_child(parent_from_child: macaw::IsoTransform) -> Self {
        Self {
            rotation: parent_from_child.rotation().into(),
            translation: parent_from_child.translation().into(),
        }
    }

    #[inline]
    pub fn new_child_from_parent(child_from_parent: macaw::IsoTransform) -> Self {
        Self::new_parent_from_child(child_from_parent.inverse())
    }

    #[inline]
    pub fn parent_from_child(&self) -> macaw::IsoTransform {
        let rotation = self.rotation.into();
        let translation = self.translation.into();
        macaw::IsoTransform::from_rotation_translation(rotation, translation)
    }

    #[inline]
    pub fn child_from_parent(&self) -> macaw::IsoTransform {
        self.parent_from_child().inverse()
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
        self.resolution.map(|r| self.image_from_cam[0][0] / r[0])
    }

    /// Principal point of the pinhole camera,
    /// i.e. the intersection of the optical axis and the image plane.
    ///
    /// [see definition of intrinsic matrix](https://en.wikipedia.org/wiki/Camera_resectioning#Intrinsic_parameters)
    #[cfg(feature = "glam")]
    #[inline]
    #[cfg(feature = "glam")]
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

/// A transform between two spaces.
///
/// ```
/// use re_log_types::component_types::{Transform, Rigid3, Pinhole};
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field, UnionMode};
///
/// assert_eq!(
///     Transform::data_type(),
///     DataType::Union(
///        vec![
///            Field::new("Unknown", DataType::Boolean, false),
///            Field::new(
///                "Rigid3",
///                Rigid3::data_type(),
///                false
///            ),
///            Field::new(
///                "Pinhole",
///                Pinhole::data_type(),
///                false
///            )
///        ],
///        None,
///        UnionMode::Dense
///     )
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum Transform {
    /// We don't know the transform, but it is likely/potentially non-identity.
    /// Maybe the user intend to set the transform later.
    Unknown,

    /// For instance: the parent is a 3D world space, the child a camera space.
    Rigid3(Rigid3),

    /// The parent is some local camera space, the child an image space.
    Pinhole(Pinhole),
}

impl Component for Transform {
    fn name() -> crate::ComponentName {
        "rerun.transform".into()
    }
}

#[test]
fn test_transform_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let transforms_in = vec![
        Transform::Pinhole(Pinhole {
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
            resolution: None,
        }),
        Transform::Rigid3(Rigid3 {
            rotation: Quaternion {
                x: 11.0,
                y: 12.0,
                z: 13.0,
                w: 14.0,
            },
            translation: [15.0, 16.0, 17.0].into(),
        }),
        Transform::Pinhole(Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]].into(),
            resolution: Some([123.0, 456.0].into()),
        }),
    ];
    let array: Box<dyn Array> = transforms_in.try_into_arrow().unwrap();
    let transforms_out: Vec<Transform> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(transforms_in, transforms_out);
}
