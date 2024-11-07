mod draw;
mod state;

pub mod canvas;

pub use state::GraphSpaceViewState;

pub fn bounding_rect_from_iter<'a>(rectangles: impl Iterator<Item = &'a egui::Rect>) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(*rect))
}
