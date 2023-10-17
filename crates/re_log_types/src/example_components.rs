//! Example components to be used for tests and docs

use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_types::Loggable as _;

// ----------------------------------------------------------------------------

pub struct MyPoints;

impl re_types::Archetype for MyPoints {
    type Indicator = re_types::GenericIndicatorComponent<Self>;

    fn name() -> re_types::ArchetypeName {
        "test.MyPoints".into()
    }

    fn required_components() -> ::std::borrow::Cow<'static, [re_types::ComponentName]> {
        vec![MyPoint::name()].into()
    }

    fn recommended_components() -> std::borrow::Cow<'static, [re_types_core::ComponentName]> {
        vec![MyColor::name(), MyLabel::name()].into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Default, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct MyPoint {
    pub x: f32,
    pub y: f32,
}

impl MyPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

use crate as re_log_types;

re_log_types::arrow2convert_component_shim!(MyPoint as "test.Point2D");

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

impl From<u32> for MyColor {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

re_log_types::arrow2convert_component_shim!(MyColor as "test.Color");

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct MyLabel(pub String);

re_log_types::arrow2convert_component_shim!(MyLabel as "test.Label");
