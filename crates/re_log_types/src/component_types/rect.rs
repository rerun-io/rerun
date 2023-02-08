use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

use super::Vec4D;

/// A rectangle in 2D space.
///
/// ## Example
/// ```
/// # use re_log_types::component_types::{Rect2D, Vec4D};
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Rect2D::data_type(),
///     DataType::Union(vec![
///         Field::new("XYWH", Vec4D::data_type(), false),
///         Field::new("YXHW", Vec4D::data_type(), false),
///         Field::new("XYXY", Vec4D::data_type(), false),
///         Field::new("YXYX", Vec4D::data_type(), false),
///         Field::new("XCYCWH", Vec4D::data_type(), false),
///         Field::new("XCYCW2H2", Vec4D::data_type(), false),
///     ], None, UnionMode::Dense)
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
#[arrow_field(type = "dense")]
pub enum Rect2D {
    /// \[x, y, w, h\], with x,y = left,top.
    XYWH(Vec4D),

    /// \[y, x, h, w\], with x,y = left,top.
    YXHW(Vec4D),

    /// \[x0, y0, x1, y1\], with x0,y0 = left,top and x1,y1 = right,bottom
    XYXY(Vec4D),

    /// \[y0, x0, y1, x1\], with x0,y0 = left,top and x1,y1 = right,bottom
    YXYX(Vec4D),

    /// \[x_center, y_center, width, height\]
    XCYCWH(Vec4D),

    /// \[x_center, y_center, width/2, height/2\]
    XCYCW2H2(Vec4D),
}

impl Rect2D {
    #[inline]
    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::XYWH(Vec4D([x, y, w, h]))
    }

    pub fn top_left_corner(&self) -> [f32; 2] {
        match self {
            Rect2D::XYWH(Vec4D([x, y, _, _])) => [*x, *y],
            Rect2D::YXHW(Vec4D([y, x, _h, _w])) => [*x, *y],
            Rect2D::XYXY(Vec4D([x0, y0, _x1, _y1])) => [*x0, *y0],
            Rect2D::YXYX(Vec4D([y0, x0, _y1, _x1])) => [*x0, *y0],
            Rect2D::XCYCWH(Vec4D([x_cen, y_cen, w, h])) => [x_cen - (w / 2.0), y_cen + (h / 2.0)],
            Rect2D::XCYCW2H2(Vec4D([x_cen, y_cen, w_2, h_2])) => [x_cen - w_2, y_cen + h_2],
        }
    }

    pub fn width(&self) -> f32 {
        match self {
            Rect2D::XYWH(Vec4D([_x, _y, w, _h])) => *w,
            Rect2D::YXHW(Vec4D([_y, _x, _h, w])) => *w,
            Rect2D::XYXY(Vec4D([x0, _y0, x1, _y1])) => x1 - x0,
            Rect2D::YXYX(Vec4D([_y0, x0, _y1, x1])) => x1 - x0,
            Rect2D::XCYCWH(Vec4D([_x_cen, _y_cen, w, _h])) => *w,
            Rect2D::XCYCW2H2(Vec4D([_x_cen, _y_cen, w_2, _h_2])) => 2.0 * w_2,
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            Rect2D::XYWH(Vec4D([_x, _y, _w, h])) => *h,
            Rect2D::YXHW(Vec4D([_y, _x, h, _w])) => *h,
            Rect2D::XYXY(Vec4D([_x0, y0, _x1, y1])) => y1 - y0,
            Rect2D::YXYX(Vec4D([y0, _x0, y1, _x1])) => y1 - y0,
            Rect2D::XCYCWH(Vec4D([_x_cen, _y_cen, _w, h])) => *h,
            Rect2D::XCYCW2H2(Vec4D([_x_cen, _y_cen, _w_2, h_2])) => 2.0 * h_2,
        }
    }
}

impl Component for Rect2D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.rect2d".into()
    }
}
