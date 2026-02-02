#[cfg(feature = "egui_plot")]
impl From<super::Corner2D> for egui_plot::Corner {
    fn from(corner: super::Corner2D) -> Self {
        match corner {
            super::Corner2D::LeftTop => Self::LeftTop,
            super::Corner2D::RightTop => Self::RightTop,
            super::Corner2D::LeftBottom => Self::LeftBottom,
            super::Corner2D::RightBottom => Self::RightBottom,
        }
    }
}

#[cfg(feature = "egui_plot")]
impl From<egui_plot::Corner> for super::Corner2D {
    fn from(corner: egui_plot::Corner) -> Self {
        match corner {
            egui_plot::Corner::LeftTop => Self::LeftTop,
            egui_plot::Corner::RightTop => Self::RightTop,
            egui_plot::Corner::LeftBottom => Self::LeftBottom,
            egui_plot::Corner::RightBottom => Self::RightBottom,
        }
    }
}
