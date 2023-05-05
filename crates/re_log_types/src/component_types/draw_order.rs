use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// Draw order used for the display order of 2D elements.
///
/// Higher values are drawn on top of lower values.
/// An entity can have only a single draw order component.
/// Within an entity draw order is governed by the order of the components.
///
/// ```
/// use re_log_types::component_types::DrawOrder;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(DrawOrder::data_type(), DataType::Float32);
/// ```
///
/// TODO: Define default ordering for different elements.
#[derive(Debug, Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct DrawOrder(pub f32);

impl DrawOrder {
    pub const DEFAULT_IMAGE: DrawOrder = DrawOrder(-10.0);
    pub const DEFAULT_BOX2D: DrawOrder = DrawOrder(10.0);
    pub const DEFAULT_LINES: DrawOrder = DrawOrder(20.0);
    pub const DEFAULT_POINTS: DrawOrder = DrawOrder(30.0);
}

impl Component for DrawOrder {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.draw_order".into()
    }
}

impl std::cmp::PartialEq for DrawOrder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.is_nan() && other.0.is_nan() || self.0 == other.0
    }
}

impl std::cmp::Eq for DrawOrder {}

impl std::cmp::PartialOrd for DrawOrder {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if other == self {
            Some(std::cmp::Ordering::Equal)
        } else if other.0.is_nan() || self.0 < other.0 {
            Some(std::cmp::Ordering::Less)
        } else {
            Some(std::cmp::Ordering::Greater)
        }
    }
}

impl std::cmp::Ord for DrawOrder {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl From<f32> for DrawOrder {
    #[inline]
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<DrawOrder> for f32 {
    #[inline]
    fn from(value: DrawOrder) -> Self {
        value.0
    }
}
