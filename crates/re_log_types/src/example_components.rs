//! Example components to be used for tests and docs

use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::{ComponentName, LegacyComponent};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct MyPoint {
    pub x: f32,
    pub y: f32,
}

impl LegacyComponent for MyPoint {
    #[inline]
    fn legacy_name() -> ComponentName {
        "test.point2d".into()
    }
}

use crate as re_log_types;

re_log_types::component_legacy_shim!(MyPoint);

// ----------------------------------------------------------------------------

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    arrow2_convert::ArrowField,
    arrow2_convert::ArrowSerialize,
    arrow2_convert::ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
#[repr(transparent)]
pub struct MyColor(pub u32);

impl LegacyComponent for MyColor {
    #[inline]
    fn legacy_name() -> ComponentName {
        "test.colorrgba".into()
    }
}

re_log_types::component_legacy_shim!(MyColor);

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct MyLabel(pub String);

impl LegacyComponent for MyLabel {
    #[inline]
    fn legacy_name() -> ComponentName {
        "test.label".into()
    }
}

re_log_types::component_legacy_shim!(MyLabel);
