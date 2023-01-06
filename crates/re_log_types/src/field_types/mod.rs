//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::datatypes::Field;
use lazy_static::lazy_static;

use crate::msg_bundle::Component;

mod class_id;
mod color;
mod instance;
mod keypoint_id;
mod label;
mod mat;
mod msg_id;
mod point;
mod quaternion;
mod radius;
mod rect;
mod scalar;
mod size;
mod text_entry;
mod transform;
mod vec;

pub use class_id::ClassId;
pub use color::ColorRGBA;
pub use instance::Instance;
pub use keypoint_id::KeypointId;
pub use label::Label;
pub use mat::Mat3x3;
pub use msg_id::MsgId;
pub use point::{Point2D, Point3D};
pub use quaternion::Quaternion;
pub use radius::Radius;
pub use rect::Rect2D;
pub use scalar::{Scalar, ScalarPlotProps};
pub use size::Size3D;
pub use text_entry::TextEntry;
pub use transform::{Pinhole, Rigid3, Transform};
pub use vec::{Vec2D, Vec3D};

lazy_static! {
    //TODO(john) actully use a run-time type registry
    static ref FIELDS: [Field; 17] = [
        <ColorRGBA as Component>::field(),
        <Instance as Component>::field(),
        <KeypointId as Component>::field(),
        <Label as Component>::field(),
        <MsgId as Component>::field(),
        <Point2D as Component>::field(),
        <Point3D as Component>::field(),
        <Quaternion as Component>::field(),
        <Radius as Component>::field(),
        <Rect2D as Component>::field(),
        <Scalar as Component>::field(),
        <ScalarPlotProps as Component>::field(),
        <Size3D as Component>::field(),
        <TextEntry as Component>::field(),
        <Transform as Component>::field(),
        <Vec2D as Component>::field(),
        <Vec3D as Component>::field(),
    ];
}

/// Iterate over the registered field types
pub fn iter_registered_field_types() -> impl Iterator<Item = &'static Field> {
    FIELDS.iter()
}
