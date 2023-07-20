use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

// TODO(cmc): Points should just be containers of Vecs.

/// A point in 2D space.
///
/// ```
/// use re_components::Point2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Point2D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const ZERO: Point2D = Point2D { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl re_log_types::LegacyComponent for Point2D {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.point2d".into()
    }
}

impl From<[f32; 2]> for Point2D {
    #[inline]
    fn from(p: [f32; 2]) -> Self {
        Self { x: p[0], y: p[1] }
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

impl re_types::Loggable for Point2D {
    type Name = re_types::ComponentName;
    type Item<'a> = Option<Point2D>;
    type IterItem<'a> =
        <&'a <Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        <Self as re_log_types::LegacyComponent>::legacy_name()
            .as_str()
            .into()
    }
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        <Self as re_log_types::LegacyComponent>::field()
            .data_type()
            .clone()
    }
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
        _extension_wrapper: Option<&str>,
    ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        let input = data.into_iter().map(|datum| {
            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
            datum.map(|d| d.into_owned())
        });
        let vec: Vec<_> = input.collect();
        let arrow = arrow2_convert::serialize::TryIntoArrow::try_into_arrow(vec.iter())
            .map_err(|err| re_types::SerializationError::ArrowConvertFailure(err.to_string()))?;
        Ok(arrow)
    }
    fn try_from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use arrow2_convert::deserialize::arrow_array_deserialize_iterator;
        let native = arrow_array_deserialize_iterator(data)
            .map_err(|err| re_types::DeserializationError::ArrowConvertFailure(err.to_string()))?
            .collect();
        Ok(native)
    }

    fn try_from_arrow_opt_iter(
        data: &dyn arrow2::array::Array,
    ) -> re_types::DeserializationResult<Self::IterItem<'_>>
    where
        Self: Sized,
    {
        let native =
               <<Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as arrow2_convert::deserialize::ArrowArray>::iter_from_array_ref(data);
        Ok(native)
    }

    fn iter_mapper(item: Self::Item<'_>) -> Option<Self> {
        <Self as arrow2_convert::deserialize::ArrowDeserialize>::arrow_deserialize(item)
    }
}
impl re_types::Component for Point2D {}

impl<'a> From<Point2D> for ::std::borrow::Cow<'a, Point2D> {
    #[inline]
    fn from(value: Point2D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}
impl<'a> From<&'a Point2D> for ::std::borrow::Cow<'a, Point2D> {
    #[inline]
    fn from(value: &'a Point2D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

/// A point in 3D space.
///
/// ```
/// use re_components::Point3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Point3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("z", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub const ZERO: Point3D = Point3D {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl re_log_types::LegacyComponent for Point3D {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.point3d".into()
    }
}

impl From<[f32; 3]> for Point3D {
    #[inline]
    fn from(p: [f32; 3]) -> Self {
        Self {
            x: p[0],
            y: p[1],
            z: p[2],
        }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Point3D {
    #[inline]
    fn from(pt: glam::Vec3) -> Self {
        Self::new(pt.x, pt.y, pt.z)
    }
}

#[cfg(feature = "glam")]
impl From<Point3D> for glam::Vec3 {
    #[inline]
    fn from(pt: Point3D) -> Self {
        Self::new(pt.x, pt.y, pt.z)
    }
}

re_log_types::component_legacy_shim!(Point3D);
