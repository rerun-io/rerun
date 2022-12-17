//! Potentially user-facing component types.
//!
//! The SDK is responsible for submitting component columns that conforms to these schemas. The
//! schemas are additionally documented in doctests.

use arrow2::datatypes::Field;
use lazy_static::lazy_static;

use crate::msg_bundle::Component;

mod color;
mod instance;
mod label;
mod point;
mod quaternion;
mod rect;
mod size;

pub use color::ColorRGBA;
pub use instance::Instance;
pub use label::Label;
pub use point::{Point2D, Point3D};
pub use quaternion::Quaternion;
pub use rect::Rect2D;
pub use size::Size3D;

lazy_static! {
    //TODO(john) actully use a run-time type registry
    static ref FIELDS: [Field; 8] = [
        <ColorRGBA as Component>::field(),
        <Instance as Component>::field(),
        <Label as Component>::field(),
        <Point2D as Component>::field(),
        <Point3D as Component>::field(),
        <Quaternion as Component>::field(),
        <Rect2D as Component>::field(),
        <Size3D as Component>::field(),
    ];
}

/// Iterate over the registered field types
pub fn iter_registered_field_types() -> impl Iterator<Item = &'static Field> {
    FIELDS.iter()
}
