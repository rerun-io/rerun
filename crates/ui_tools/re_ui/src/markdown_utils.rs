use std::fmt::{Display, Formatter};

/// Newtype over [`egui::PointerButton`] which provides a [`Display`] implementation suitable for
/// markdown.
pub struct MouseButtonMarkdown(pub egui::PointerButton);

impl Display for MouseButtonMarkdown {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            egui::PointerButton::Primary => write!(f, "`left` mouse button"),
            egui::PointerButton::Secondary => write!(f, "`right` mouse button"),
            egui::PointerButton::Middle => write!(f, "`middle` mouse button"),
            egui::PointerButton::Extra1 => write!(f, "`extra 1` mouse button"),
            egui::PointerButton::Extra2 => write!(f, "`extra 2` mouse button"),
        }
    }
}

/// Newtype over [`egui::Key`] which provides a [`Display`] implementation suitable for markdown.
pub struct KeyMarkdown(pub egui::Key);

impl Display for KeyMarkdown {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}`", self.0.name())
    }
}
