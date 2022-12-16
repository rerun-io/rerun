//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::{
    array::TryPush,
    datatypes::{DataType, Field},
};
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize, ArrowField,
};
use lazy_static::lazy_static;

use crate::msg_bundle::Component;

lazy_static! {
    //TODO(john) actully use a run-time type registry
    static ref FIELDS: [Field; 4] = [
        <ColorRGBA as Component>::field(),
        <Point2D as Component>::field(),
        <Point3D as Component>::field(),
        <Rect2D as Component>::field(),
    ];
}

/// Iterate over the registered field types
pub fn iter_registered_field_types() -> impl Iterator<Item = &'static Field> {
    FIELDS.iter()
}

/// The Instance used to identify an entity within a batch
///
/// ```
/// use re_log_types::field_types::Instance;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Instance::data_type(), DataType::UInt64);
/// ```
#[derive(Debug)]
pub struct Instance(pub u64);

arrow_enable_vec_for_type!(Instance);

impl ArrowField for Instance {
    type Type = Self;
    fn data_type() -> DataType {
        <u64 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Instance {
    type MutableArrayType = <u64 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for Instance {
    type ArrayType = <u64 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u64 as ArrowDeserialize>::arrow_deserialize(v).map(Instance)
    }
}

impl Component for Instance {
    const NAME: crate::ComponentNameRef<'static> = "instance";
}

/// A rectangle in 2D space.
///
/// ```
/// use re_log_types::field_types::Rect2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Rect2D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("w", DataType::Float32, false),
///         Field::new("h", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Debug, ArrowField)]
pub struct Rect2D {
    /// Rect X-coordinate
    pub x: f32,
    /// Rect Y-coordinate
    pub y: f32,
    /// Box Width
    pub w: f32,
    /// Box Height
    pub h: f32,
}

impl Component for Rect2D {
    const NAME: crate::ComponentNameRef<'static> = "rect2d";
}

/// A point in 2D space.
///
/// ```
/// use re_log_types::field_types::Point2D;
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
#[derive(Debug, ArrowField)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Component for Point2D {
    const NAME: crate::ComponentNameRef<'static> = "point2d";
}

/// A point in 3D space.
///
/// ```
/// use re_log_types::field_types::Point3D;
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
#[derive(Debug, ArrowField)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Point3D {
    const NAME: crate::ComponentNameRef<'static> = "point3d";
}

/// An RGBA color tuple.
///
/// ```
/// use re_log_types::field_types::ColorRGBA;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(ColorRGBA::data_type(), DataType::UInt32);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ColorRGBA(pub u32);

arrow_enable_vec_for_type!(ColorRGBA);

impl ArrowField for ColorRGBA {
    type Type = Self;
    fn data_type() -> DataType {
        <u32 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for ColorRGBA {
    type MutableArrayType = <u32 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for ColorRGBA {
    type ArrayType = <u32 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u32 as ArrowDeserialize>::arrow_deserialize(v).map(ColorRGBA)
    }
}

impl Component for ColorRGBA {
    const NAME: crate::ComponentNameRef<'static> = "colorrgba";
}

#[test]
fn test_colorrgba_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let colors_in = vec![ColorRGBA(0u32), ColorRGBA(255u32)];
    let array: Box<dyn Array> = colors_in.try_into_arrow().unwrap();
    let colors_out: Vec<ColorRGBA> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(colors_in, colors_out);
}
