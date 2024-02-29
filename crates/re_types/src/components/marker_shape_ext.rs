#[cfg(feature = "egui_plot")]
use super::MarkerShape;

#[cfg(feature = "egui_plot")]
impl From<egui_plot::MarkerShape> for MarkerShape {
    #[inline]
    fn from(shape: egui_plot::MarkerShape) -> Self {
        match shape {
            egui_plot::MarkerShape::Circle => Self::Circle,
            egui_plot::MarkerShape::Diamond => Self::Diamond,
            egui_plot::MarkerShape::Square => Self::Square,
            egui_plot::MarkerShape::Cross => Self::Cross,
            egui_plot::MarkerShape::Plus => Self::Plus,
            egui_plot::MarkerShape::Up => Self::Up,
            egui_plot::MarkerShape::Down => Self::Down,
            egui_plot::MarkerShape::Left => Self::Left,
            egui_plot::MarkerShape::Right => Self::Right,
            egui_plot::MarkerShape::Asterisk => Self::Asterisk,
        }
    }
}

#[cfg(feature = "egui_plot")]
impl From<MarkerShape> for egui_plot::MarkerShape {
    #[inline]
    fn from(shape: MarkerShape) -> Self {
        match shape {
            MarkerShape::Circle => Self::Circle,
            MarkerShape::Diamond => Self::Diamond,
            MarkerShape::Square => Self::Square,
            MarkerShape::Cross => Self::Cross,
            MarkerShape::Plus => Self::Plus,
            MarkerShape::Up => Self::Up,
            MarkerShape::Down => Self::Down,
            MarkerShape::Left => Self::Left,
            MarkerShape::Right => Self::Right,
            MarkerShape::Asterisk => Self::Asterisk,
        }
    }
}
