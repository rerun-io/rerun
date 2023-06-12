// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

/// A unique numeric identifier for each individual instance within a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstanceKey(pub u64);

impl crate::Component for InstanceKey {
    fn name() -> ::std::borrow::Cow<'static, str> {
        ::std::borrow::Cow::Borrowed("rerun.components.InstanceKey")
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::UInt64
    }
}
