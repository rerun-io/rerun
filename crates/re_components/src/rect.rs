#![allow(clippy::upper_case_acronyms)]

use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use super::LegacyVec4D;

/// TODO(#3243): Legacy component still used in some tests, benchmarks and examples.
///
/// A rectangle in 2D space.
///
/// ## Example
/// ```
/// # use re_components::{Rect2D, LegacyVec4D};
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Rect2D::data_type(),
///     DataType::Union(vec![
///         Field::new("XYWH", LegacyVec4D::data_type(), false),
///         Field::new("YXHW", LegacyVec4D::data_type(), false),
///         Field::new("XYXY", LegacyVec4D::data_type(), false),
///         Field::new("YXYX", LegacyVec4D::data_type(), false),
///         Field::new("XCYCWH", LegacyVec4D::data_type(), false),
///         Field::new("XCYCW2H2", LegacyVec4D::data_type(), false),
///     ], None, UnionMode::Dense)
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
#[arrow_field(type = "dense")]
pub enum Rect2D {
    /// \[x, y, w, h\], with x,y = left,top.
    XYWH(LegacyVec4D),

    /// \[y, x, h, w\], with x,y = left,top.
    YXHW(LegacyVec4D),

    /// \[x0, y0, x1, y1\], with x0,y0 = left,top and x1,y1 = right,bottom
    XYXY(LegacyVec4D),

    /// \[y0, x0, y1, x1\], with x0,y0 = left,top and x1,y1 = right,bottom
    YXYX(LegacyVec4D),

    /// \[x_center, y_center, width, height\]
    XCYCWH(LegacyVec4D),

    /// \[x_center, y_center, width/2, height/2\]
    XCYCW2H2(LegacyVec4D),
}

impl Rect2D {
    #[inline]
    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::XYWH(LegacyVec4D([x, y, w, h]))
    }

    pub fn top_left_corner(&self) -> [f32; 2] {
        match self {
            Rect2D::XYWH(LegacyVec4D([x, y, _, _])) => [*x, *y],
            Rect2D::YXHW(LegacyVec4D([y, x, _h, _w])) => [*x, *y],
            Rect2D::XYXY(LegacyVec4D([x0, y0, _x1, _y1])) => [*x0, *y0],
            Rect2D::YXYX(LegacyVec4D([y0, x0, _y1, _x1])) => [*x0, *y0],
            Rect2D::XCYCWH(LegacyVec4D([x_cen, y_cen, w, h])) => {
                [x_cen - (w / 2.0), y_cen - (h / 2.0)]
            }
            Rect2D::XCYCW2H2(LegacyVec4D([x_cen, y_cen, w_2, h_2])) => [x_cen - w_2, y_cen - h_2],
        }
    }

    pub fn width(&self) -> f32 {
        match self {
            Rect2D::XYWH(LegacyVec4D([_x, _y, w, _h])) => *w,
            Rect2D::YXHW(LegacyVec4D([_y, _x, _h, w])) => *w,
            Rect2D::XYXY(LegacyVec4D([x0, _y0, x1, _y1])) => x1 - x0,
            Rect2D::YXYX(LegacyVec4D([_y0, x0, _y1, x1])) => x1 - x0,
            Rect2D::XCYCWH(LegacyVec4D([_x_cen, _y_cen, w, _h])) => *w,
            Rect2D::XCYCW2H2(LegacyVec4D([_x_cen, _y_cen, w_2, _h_2])) => 2.0 * w_2,
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            Rect2D::XYWH(LegacyVec4D([_x, _y, _w, h])) => *h,
            Rect2D::YXHW(LegacyVec4D([_y, _x, h, _w])) => *h,
            Rect2D::XYXY(LegacyVec4D([_x0, y0, _x1, y1])) => y1 - y0,
            Rect2D::YXYX(LegacyVec4D([y0, _x0, y1, _x1])) => y1 - y0,
            Rect2D::XCYCWH(LegacyVec4D([_x_cen, _y_cen, _w, h])) => *h,
            Rect2D::XCYCW2H2(LegacyVec4D([_x_cen, _y_cen, _w_2, h_2])) => 2.0 * h_2,
        }
    }
}

impl re_log_types::LegacyComponent for Rect2D {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.rect2d".into()
    }
}

re_log_types::component_legacy_shim!(Rect2D);
