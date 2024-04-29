//! Second-generation list item.
//!
//! TODO(ab): provide some top-level documentation here.

mod label_content;
mod list_item;
mod other_contents;
mod scope;

pub use label_content::*;
pub use list_item::*;
pub use other_contents::*;
pub use scope::*;

/// Context provided to [`ListItemContent`] implementations
#[derive(Debug, Clone)]
pub struct ContentContext<'a> {
    /// Area available for the content to draw in.
    pub rect: egui::Rect,

    /// Background area
    pub bg_rect: egui::Rect,

    /// List item response.
    ///
    /// Note: this response reflects the hover state when [`ListItem::force_hovered`] is used,
    /// regardless of the actual mouse position.
    pub response: &'a egui::Response,

    /// The current list item.
    pub list_item: &'a ListItem<'a>,

    /// The frame-over-frame state for this list item.
    pub state: &'a State,
}

#[derive(Debug, Clone, Copy)]
pub enum DesiredWidth {
    /// The content needs exactly this width for display.
    ///
    /// This mode is useful when it is important to not overallocate (e.g. streams view).
    Exact(f32),

    /// The content needs at least this width for display, but will use more if available.
    ///
    /// In this mode, [`ListItem`] will try to allocate this much width in addition to indent and
    /// collapsing triangle (if any). This may trigger some scrolling. In any case, the content will
    /// be provided with `ui.available_width()`.
    AtLeast(f32),
}

impl Default for DesiredWidth {
    fn default() -> Self {
        DesiredWidth::AtLeast(0.0)
    }
}

pub trait ListItemContent {
    /// UI for everything that is after the indent and the collapsing triangle (if any).
    ///
    /// The content should render within the provided `context.rect`.
    ///
    /// If the content has some interactive elements, it should return its response. In particular,
    /// if the response is hovered, the list item will show a dimmer background highlight.
    fn ui(
        self: Box<Self>,
        re_ui: &crate::ReUi,
        ui: &mut egui::Ui,
        context: &ContentContext<'_>,
    ) -> Option<egui::Response>;

    /// The desired width of the content.
    fn desired_width(&self, _re_ui: &crate::ReUi, _ui: &egui::Ui) -> DesiredWidth {
        DesiredWidth::AtLeast(0.0)
    }
}
