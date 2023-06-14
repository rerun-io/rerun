// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

/// An RGBA color tuple with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(transparent)]
pub struct Color(pub u32);

impl crate::Component for Color {
    fn name() -> crate::ComponentName {
        crate::ComponentName::Borrowed("rerun.components.Color")
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::UInt32
    }
}
