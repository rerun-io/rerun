//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::{
    array::{FixedSizeListArray, MutableFixedSizeListArray, PrimitiveArray},
    datatypes::{DataType, Field},
};
use arrow2_convert::{
    deserialize::{ArrowArray, ArrowDeserialize},
    field::{ArrowEnableVecForType, ArrowField},
    serialize::ArrowSerialize,
};
use lazy_static::lazy_static;

use crate::Component;

mod arrow;
mod arrow_convert_shims;
mod bbox;
mod class_id;
mod color;
pub mod context;
pub mod coordinates;
mod imu;
mod instance_key;
mod keypoint_id;
mod label;
mod linestrip;
mod mat;
mod mesh3d;
mod node_graph;
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
mod xlink_stats;

pub use arrow::Arrow3D;
pub use bbox::Box3D;
pub use class_id::ClassId;
pub use color::ColorRGBA;
pub use context::{AnnotationContext, AnnotationInfo, ClassDescription};
pub use coordinates::ViewCoordinates;
pub use imu::ImuData;
pub use instance_key::InstanceKey;
pub use keypoint_id::KeypointId;
pub use label::Label;
pub use linestrip::{LineStrip2D, LineStrip3D};
pub use mat::Mat3x3;
pub use mesh3d::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use node_graph::NodeGraph;
pub use point::{Point2D, Point3D};
pub use quaternion::Quaternion;
pub use radius::Radius;
pub use rect::Rect2D;
pub use scalar::{Scalar, ScalarPlotProps};
pub use size::Size3D;
pub use tensor::{
    DecodedTensor, Tensor, TensorCastError, TensorData, TensorDataMeaning, TensorDimension,
    TensorId,
};
#[cfg(feature = "image")]
pub use tensor::{TensorImageLoadError, TensorImageSaveError};
pub use text_entry::TextEntry;
pub use transform::{Pinhole, Rigid3, Transform};
pub use vec::{Vec2D, Vec3D, Vec4D};
pub use xlink_stats::XlinkStats;

lazy_static! {
    //TODO(john): use a run-time type registry
    static ref FIELDS: [Field; 28] = [
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
        <NodeGraph as Component>::field(),
        <ImuData as Component>::field(),
        <XlinkStats as Component>::field(),
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
///     #[arrow_field(type = "FixedSizeArrayField<u32,2>")]
///     data: [u32; 2],
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

pub struct FastFixedSizeArrayIter<'a, T, const SIZE: usize>
where
    T: arrow2::types::NativeType,
{
    offset: usize,
    end: usize,
    array: &'a FixedSizeListArray,
    values: &'a PrimitiveArray<T>,
}

impl<'a, T, const SIZE: usize> Iterator for FastFixedSizeArrayIter<'a, T, SIZE>
where
    T: arrow2::types::NativeType,
{
    type Item = Option<[T; SIZE]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.end {
            if let Some(validity) = self.array.validity() {
                if !validity.get_bit(self.offset) {
                    self.offset += 1;
                    return Some(None);
                }
            }

            let out: [T; SIZE] =
                array_init::array_init(|i: usize| self.values.value(self.offset * SIZE + i));
            self.offset += 1;
            Some(Some(out))
        } else {
            None
        }
    }
}

pub struct FastFixedSizeListArray<T, const SIZE: usize>(std::marker::PhantomData<T>);

#[cfg(not(target_os = "windows"))]
extern "C" {
    fn do_not_call_into_iter(); // we never define this function, so the linker will fail
}

impl<'a, T, const SIZE: usize> IntoIterator for &'a FastFixedSizeListArray<T, SIZE>
where
    T: arrow2::types::NativeType,
{
    type Item = Option<[T; SIZE]>;

    type IntoIter = FastFixedSizeArrayIter<'a, T, SIZE>;

    #[cfg(not(target_os = "windows"))]
    fn into_iter(self) -> Self::IntoIter {
        #[allow(unsafe_code)]
        // SAFETY:
        // This exists so we get a link-error if some code tries to call into_iter
        // Iteration should only happen via iter_from_array_ref.
        // This is a quirk of the way the traits work in arrow2_convert.
        unsafe {
            do_not_call_into_iter();
        }
        unreachable!();
    }

    // On windows the above linker trick doesn't work.
    // We'll still catch the issue on build in Linux, but on windows just fall back to panic.
    #[cfg(target_os = "windows")]
    fn into_iter(self) -> Self::IntoIter {
        panic!("Use iter_from_array_ref. This is a quirk of the way the traits work in arrow2_convert.");
    }
}

impl<T, const SIZE: usize> ArrowArray for FastFixedSizeListArray<T, SIZE>
where
    T: arrow2::types::NativeType,
{
    type BaseArrayType = FixedSizeListArray;

    fn iter_from_array_ref(b: &dyn arrow2::array::Array) -> <&Self as IntoIterator>::IntoIter {
        let array = b.as_any().downcast_ref::<Self::BaseArrayType>().unwrap();
        let values = array
            .values()
            .as_any()
            .downcast_ref::<PrimitiveArray<T>>()
            .unwrap();
        FastFixedSizeArrayIter::<T, SIZE> {
            offset: 0,
            end: array.len(),
            array,
            values,
        }
    }
}

impl<T, const SIZE: usize> ArrowDeserialize for FixedSizeArrayField<T, SIZE>
where
    T: arrow2::types::NativeType
        + ArrowDeserialize
        + ArrowEnableVecForType
        + ArrowField<Type = T>
        + 'static,
    <T as ArrowDeserialize>::ArrayType: 'static,
    for<'b> &'b <T as ArrowDeserialize>::ArrayType: IntoIterator,
{
    type ArrayType = FastFixedSizeListArray<T, SIZE>;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v
    }
}
