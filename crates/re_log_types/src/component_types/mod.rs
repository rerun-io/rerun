//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::{
    array::{FixedSizeListArray, MutableFixedSizeListArray},
    datatypes::{DataType, Field},
};
use arrow2_convert::{
    deserialize::{ArrowArray, ArrowDeserialize},
    field::{ArrowEnableVecForType, ArrowField},
    serialize::ArrowSerialize,
};
use lazy_static::lazy_static;

use crate::msg_bundle::Component;

mod arrow;
mod bbox;
mod class_id;
mod color;
pub mod context;
pub mod coordinates;
mod instance_key;
mod keypoint_id;
mod label;
mod linestrip;
mod mat;
mod mesh3d;
mod msg_id;
mod point;
mod quaternion;
mod radius;
mod rect;
mod scalar;
mod size;
mod tensor;
mod text_entry;
mod transform;
mod vec;

pub use arrow::Arrow3D;
pub use bbox::Box3D;
pub use class_id::ClassId;
pub use color::ColorRGBA;
pub use context::{AnnotationContext, AnnotationInfo, ClassDescription};
pub use coordinates::ViewCoordinates;
pub use instance_key::InstanceKey;
pub use keypoint_id::KeypointId;
pub use label::Label;
pub use linestrip::{LineStrip2D, LineStrip3D};
pub use mat::Mat3x3;
pub use mesh3d::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use msg_id::MsgId;
pub use point::{Point2D, Point3D};
pub use quaternion::Quaternion;
pub use radius::Radius;
pub use rect::Rect2D;
pub use scalar::{Scalar, ScalarPlotProps};
pub use size::Size3D;
pub use tensor::{Tensor, TensorData, TensorDataMeaning, TensorDimension, TensorId, TensorTrait};
pub use text_entry::TextEntry;
pub use transform::{Pinhole, Rigid3, Transform};
pub use vec::{Vec2D, Vec3D, Vec4D};

lazy_static! {
    //TODO(john): use a run-time type registry
    static ref FIELDS: [Field; 26] = [
        <AnnotationContext as Component>::field(),
        <Arrow3D as Component>::field(),
        <Box3D as Component>::field(),
        <ClassId as Component>::field(),
        <ColorRGBA as Component>::field(),
        <InstanceKey as Component>::field(),
        <KeypointId as Component>::field(),
        <Label as Component>::field(),
        <LineStrip2D as Component>::field(),
        <LineStrip3D as Component>::field(),
        <Mesh3D as Component>::field(),
        <MsgId as Component>::field(),
        <Point2D as Component>::field(),
        <Point3D as Component>::field(),
        <Quaternion as Component>::field(),
        <Radius as Component>::field(),
        <Rect2D as Component>::field(),
        <Scalar as Component>::field(),
        <ScalarPlotProps as Component>::field(),
        <Size3D as Component>::field(),
        <Tensor as Component>::field(),
        <TextEntry as Component>::field(),
        <Transform as Component>::field(),
        <Vec2D as Component>::field(),
        <Vec3D as Component>::field(),
        <ViewCoordinates as Component>::field(),
    ];
}

/// Iterate over the registered field types
pub fn iter_registered_field_types() -> impl Iterator<Item = &'static Field> {
    FIELDS.iter()
}

#[derive(thiserror::Error, Debug)]
pub enum FieldError {
    #[error("Encountered bad value")]
    BadValue,

    #[error("Slice over bad indices")]
    BadSlice(#[from] std::array::TryFromSliceError),
}

pub type Result<T> = std::result::Result<T, FieldError>;

/// `arrow2_convert` helper for fields of type `[T; SIZE]`
///
/// This allows us to use fields of type `[T; SIZE]` in `arrow2_convert`. Since this is a helper,
/// it must be specified as the type of the field using the `#[arrow_field(type = "FixedSizeArrayField<T,SIZE>")]` attribute.
///
/// ## Example:
/// ```
/// use arrow2_convert::{ArrowField, ArrowSerialize, ArrowDeserialize};
/// use re_log_types::component_types::FixedSizeArrayField;
///
/// #[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
/// pub struct ConvertibleType {
///     #[arrow_field(type = "FixedSizeArrayField<bool,2>")]
///     data: [bool; 2],
/// }
/// ```
pub struct FixedSizeArrayField<T, const SIZE: usize>(std::marker::PhantomData<T>);

impl<T, const SIZE: usize> ArrowField for FixedSizeArrayField<T, SIZE>
where
    T: ArrowField + ArrowEnableVecForType,
{
    type Type = [T; SIZE];

    #[inline]
    fn data_type() -> DataType {
        arrow2::datatypes::DataType::FixedSizeList(Box::new(<T as ArrowField>::field("item")), SIZE)
    }
}

impl<T, const SIZE: usize> ArrowSerialize for FixedSizeArrayField<T, SIZE>
where
    T: ArrowSerialize + ArrowEnableVecForType + ArrowField<Type = T> + 'static,
    <T as ArrowSerialize>::MutableArrayType: Default,
{
    type MutableArrayType = MutableFixedSizeListArray<<T as ArrowSerialize>::MutableArrayType>;
    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::new_with_field(
            <T as ArrowSerialize>::new_array(),
            "item",
            <T as ArrowField>::is_nullable(),
            SIZE,
        )
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        let values = array.mut_values();
        for i in v.iter() {
            <T as ArrowSerialize>::arrow_serialize(i, values)?;
        }
        array.try_push_valid()
    }
}

impl<T, const SIZE: usize> ArrowDeserialize for FixedSizeArrayField<T, SIZE>
where
    T: ArrowDeserialize + ArrowEnableVecForType + ArrowField<Type = T> + 'static,
    <T as ArrowDeserialize>::ArrayType: 'static,
    for<'b> &'b <T as ArrowDeserialize>::ArrayType: IntoIterator,
{
    type ArrayType = FixedSizeListArray;

    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        if let Some(array) = v {
            let mut iter = <<T as ArrowDeserialize>::ArrayType as ArrowArray>::iter_from_array_ref(
                array.as_ref(),
            )
            .map(<T as ArrowDeserialize>::arrow_deserialize_internal);
            let out: Result<[T; SIZE]> =
                array_init::try_array_init(|_i: usize| iter.next().ok_or(FieldError::BadValue));
            out.ok()
        } else {
            None
        }
    }
}
