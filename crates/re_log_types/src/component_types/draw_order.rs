use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// Draw order used for the display order of 2D elements.
///
/// Higher values are drawn on top of lower values.
/// An entity can have only a single draw order component.
/// Within an entity draw order is governed by the order of the components.
///
/// Draw order for entities with the same draw order is generally undefined.
///
/// This component is a "mono-component". See [the crate level docs](crate) for details.
///
/// ```
/// use re_log_types::component_types::DrawOrder;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(DrawOrder::data_type(), DataType::Float32);
/// ```
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct DrawOrder(pub f32);

impl DrawOrder {
    /// Draw order used for images if no draw order was specified.
    pub const DEFAULT_IMAGE: DrawOrder = DrawOrder(-10.0);

    /// Draw order used for 2D boxes if no draw order was specified.
    pub const DEFAULT_BOX2D: DrawOrder = DrawOrder(10.0);

    /// Draw order used for 2D lines if no draw order was specified.
    pub const DEFAULT_LINES2D: DrawOrder = DrawOrder(20.0);

    /// Draw order used for 2D points if no draw order was specified.
    pub const DEFAULT_POINTS2D: DrawOrder = DrawOrder(30.0);
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
