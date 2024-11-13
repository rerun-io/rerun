mod draw;
mod state;

pub mod canvas;

pub use state::GraphSpaceViewState;

pub fn bounding_rect_from_iter(rectangles: impl Iterator<Item = egui::Rect>) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(rect))
}
