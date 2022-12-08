//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::{array::TryPush, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type, field::ArrowField, serialize::ArrowSerialize, ArrowField,
};

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
#[derive(Debug, PartialEq, ArrowField)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
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

/// An RGBA color tuple.
///
/// ```
/// use re_log_types::field_types::ColorRGBA;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(ColorRGBA::data_type(), DataType::UInt32);
/// ```
#[derive(Debug)]
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
