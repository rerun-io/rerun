//! Second-generation list item.
//!
//! TODO(ab): provide some top-level documentation here.

mod button_content;
mod custom_content;
mod debug_content;
mod item_buttons;
mod label_content;
#[expect(clippy::module_inception)]
mod list_item;
mod navigation;
mod property_content;
mod scope;

pub use button_content::*;
pub use custom_content::*;
pub use debug_content::*;
pub use item_buttons::*;
pub use label_content::*;
pub use list_item::*;
pub use property_content::*;
pub use scope::*;

/// Context provided to [`ListItemContent`] implementations
#[derive(Debug, Clone)]
pub struct ContentContext<'a> {
    /// Area available for the content to draw in.
    pub rect: egui::Rect,

    /// Background area
    ///
    /// This is the area covered by the full-span highlighting. Useful for testing if the cursor is
    /// over the item.
    pub bg_rect: egui::Rect,

    /// List item response.
    ///
    /// Note: this response reflects the hover state when [`ListItem::force_hovered`] is used,
    /// regardless of the actual mouse position.
    pub response: &'a egui::Response,

    /// The current list item.
    pub list_item: &'a ListItem,

    /// Layout information to use for rendering.
    pub layout_info: LayoutInfo,

    /// The colors to use for rendering.
    pub visuals: ListVisuals,
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
        Self::AtLeast(0.0)
    }
}

pub trait ListItemContent {
    /// UI for everything that is after the indent and the collapsing triangle (if any).
    ///
    /// The content should render within the provided `context.rect`.
    ///
    /// If the content has some interactive elements, it should return its response. In particular,
    /// if the response is hovered, the list item will show a dimmer background highlight.
    //TODO(ab): could the return type be just a bool meaning "inner interactive widget was hovered"?
    fn ui(self: Box<Self>, ui: &mut egui::Ui, context: &ContentContext<'_>);

    /// The desired width of the content.
    fn desired_width(&self, _ui: &egui::Ui) -> DesiredWidth {
        DesiredWidth::AtLeast(0.0)
    }
}
