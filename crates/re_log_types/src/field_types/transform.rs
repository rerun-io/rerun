use arrow2::{
    array::{
        Array, ArrayValuesIter, FixedSizeListArray, MutableArray, MutableFixedSizeListArray,
        MutablePrimitiveArray, MutableStructArray, PrimitiveArray,
    },
    bitmap::utils::{BitmapIter, ZipValidity},
    datatypes::{DataType, Field},
};
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize,
};

use crate::msg_bundle::Component;

use super::{Quaternion, Vec3D};

/// A proper rigid 3D transform, i.e. a rotation and a translation.
///
/// Also known as an isometric transform, or a pose.
#[derive(Copy, Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Rigid3 {
    /// How is the child rotated?
    ///
    /// This transforms to parent-space from child-space.
    rotation: Quaternion,

    /// Translation to parent from child.
    ///
    /// You can also think of this as the position of the child.
    translation: Vec3D,
}

#[cfg(feature = "glam")]
impl Rigid3 {
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
#[derive(Copy, Clone, Debug, PartialEq)]
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
    pub image_from_cam: [[f32; 3]; 3],

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// [`Self::image_from_cam`] project onto the space spanned by `(0,0)` and `resolution - 1`.
    pub resolution: Option<[f32; 2]>,
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
    #[cfg(feature = "glam")]
    pub fn focal_length_in_pixels(&self) -> glam::Vec2 {
        glam::vec2(self.image_from_cam[0][0], self.image_from_cam[1][1])
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
        self.resolution.map(|r| glam::vec2(r[0], r[1]))
    }

    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        self.image_from_cam[0][0] / self.image_from_cam[1][1]
    }
}

// ----------------------------------------------------------------------------
// Arrow2 serialization for Pinhole

arrow_enable_vec_for_type!(Pinhole);

impl ArrowField for Pinhole {
    type Type = Self;
    fn data_type() -> DataType {
        DataType::Struct(vec![
            Field::new(
                "image_from_cam",
                DataType::FixedSizeList(Box::new(Field::new("elem", DataType::Float32, false)), 9),
                false,
            ),
            Field::new(
                "resolution",
                DataType::FixedSizeList(Box::new(Field::new("elem", DataType::Float32, false)), 2),
                true,
            ),
        ])
    }
}

impl ArrowSerialize for Pinhole {
    type MutableArrayType = arrow2::array::MutableStructArray;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        let empty_mat = MutablePrimitiveArray::<f32>::new();
        let empty_res = MutablePrimitiveArray::<f32>::new();
        let image_from_cal = Box::new(MutableFixedSizeListArray::new_with_field(
            empty_mat, "elem", false, 9,
        ));
        let resolution = Box::new(MutableFixedSizeListArray::new_with_field(
            empty_res, "elem", false, 2,
        ));
        MutableStructArray::new(Pinhole::data_type(), vec![image_from_cal, resolution])
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        let Self {
            image_from_cam,
            resolution,
        } = v;

        let cam_list = array
            .mut_values()
            .get_mut(0)
            .ok_or_else(|| {
                arrow2::error::Error::ExternalFormat("Bad conversion for Pinhole".to_owned())
            })?
            .as_mut_any()
            .downcast_mut::<MutableFixedSizeListArray<MutablePrimitiveArray<f32>>>()
            .ok_or_else(|| {
                arrow2::error::Error::ExternalFormat("Bad conversion for Pinhole".to_owned())
            })?;
        let cam_values: &mut MutablePrimitiveArray<f32> = cam_list.mut_values();
        cam_values.extend_from_slice(image_from_cam[0].as_slice());
        cam_values.extend_from_slice(image_from_cam[1].as_slice());
        cam_values.extend_from_slice(image_from_cam[2].as_slice());
        cam_list.try_push_valid()?;

        let res_list = array
            .mut_values()
            .get_mut(1)
            .ok_or_else(|| {
                arrow2::error::Error::ExternalFormat("Bad conversion for Pinhole".to_owned())
            })?
            .as_mut_any()
            .downcast_mut::<MutableFixedSizeListArray<MutablePrimitiveArray<f32>>>()
            .ok_or_else(|| {
                arrow2::error::Error::ExternalFormat("Bad conversion for Pinhole".to_owned())
            })?;
        let res_values: &mut MutablePrimitiveArray<f32> = res_list.mut_values();
        if let Some(res) = resolution {
            res_values.extend(res.iter().map(|v| Some(*v)));
            res_list.try_push_valid()?;
        } else {
            res_list.push_null();
        }

        array.push(true);
        Ok(())
    }
}

/// Helper for deserializing a Pinhole camera transform
pub struct PinholeArray {}

#[allow(clippy::unimplemented)]
impl<'a> IntoIterator for &'a PinholeArray {
    type Item = Option<Pinhole>;
    type IntoIter = PinholeArrayIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        // Following the pattern established inside arrow2-convert
        unimplemented!("Use iter_from_array_ref"); // NOLINT
    }
}

impl arrow2_convert::deserialize::ArrowArray for PinholeArray {
    type BaseArrayType = arrow2::array::StructArray;
    #[inline]
    fn iter_from_array_ref(b: &dyn arrow2::array::Array) -> PinholeArrayIterator<'_> {
        // TODO(jleibs): Would be nice to avoid these unwraps but it seems like arrow2-convert
        // hasn't left us much of an option.
        let struct_arr = b.as_any().downcast_ref::<Self::BaseArrayType>().unwrap();

        let cam_iter = struct_arr
            .values()
            .get(0)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .unwrap()
            .into_iter();

        let res_iter = struct_arr
            .values()
            .get(1)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .unwrap()
            .into_iter();

        PinholeArrayIterator { cam_iter, res_iter }
    }
}
pub struct PinholeArrayIterator<'a> {
    cam_iter: ZipValidity<Box<dyn Array>, ArrayValuesIter<'a, FixedSizeListArray>, BitmapIter<'a>>,
    res_iter: ZipValidity<Box<dyn Array>, ArrayValuesIter<'a, FixedSizeListArray>, BitmapIter<'a>>,
}

impl<'a> Iterator for PinholeArrayIterator<'a> {
    type Item = Option<Pinhole>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // If either iterator runs out, stop iterating
        let next_cam = self.cam_iter.next()?;
        let next_res = self.res_iter.next()?;

        let cam_values = next_cam.expect("Cam projection cannot be null");
        let cam_slice = cam_values
            .as_any()
            .downcast_ref::<PrimitiveArray<f32>>()
            .unwrap()
            .values()
            .as_slice();

        let image_from_cam = [
            cam_slice[0..3].try_into().unwrap(),
            cam_slice[3..6].try_into().unwrap(),
            cam_slice[6..9].try_into().unwrap(),
        ];

        let resolution = if let Some(res) = next_res {
            let res_slice = res
                .as_any()
                .downcast_ref::<PrimitiveArray<f32>>()
                .unwrap()
                .values()
                .as_slice();

            Some(res_slice[0..2].try_into().unwrap())
        } else {
            None
        };

        Some(Some(Pinhole {
            image_from_cam,
            resolution,
        }))
    }
}

impl ArrowDeserialize for Pinhole {
    type ArrayType = PinholeArray;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v
    }
}

// ----------------------------------------------------------------------------

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
            image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            resolution: None,
        }),
        Transform::Rigid3(Rigid3 {
            rotation: Quaternion {
                x: 11.0,
                y: 12.0,
                z: 13.0,
                w: 14.0,
            },
            translation: Vec3D {
                x: 15.0,
                y: 16.0,
                z: 17.0,
            },
        }),
        Transform::Pinhole(Pinhole {
            image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]],
            resolution: Some([123.0, 456.0]),
        }),
    ];
    let array: Box<dyn Array> = transforms_in.try_into_arrow().unwrap();
    let transforms_out: Vec<Transform> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(transforms_in, transforms_out);
}
