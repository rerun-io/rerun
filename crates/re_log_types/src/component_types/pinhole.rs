use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

use super::{mat::Mat3x3, Vec2D};

/// Camera perspective projection (a.k.a. intrinsics).
///
/// This component is a "mono-component". See [the crate level docs](crate) for details.
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

impl Component for Pinhole {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.pinhole".into()
    }
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

#[test]
fn test_pinhole_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let pinholes_in = vec![
        Pinhole {
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
            resolution: None,
        },
        Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]].into(),
            resolution: Some([123.0, 456.0].into()),
        },
    ];
    let array: Box<dyn Array> = pinholes_in.try_into_arrow().unwrap();
    let pinholes_out: Vec<Pinhole> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(pinholes_in, pinholes_out);
}
