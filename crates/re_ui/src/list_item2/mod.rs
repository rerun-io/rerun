mod basic;
mod debug;
mod list_item;
mod scope;

pub use basic::*;
pub use debug::*;
pub use list_item::*;
pub use scope::*;

struct ContentResponse<ContRet> {
    data: ContRet,
    response: egui::Response,
}

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
    /// UI for everything that is after the indent and the collapsing triangle (if any)
    fn ui(self: Box<Self>, re_ui: &crate::ReUi, ui: &mut egui::Ui, context: &ContentContext<'_>);

    fn desired_width(&self, _re_ui: &crate::ReUi, _ui: &egui::Ui) -> DesiredWidth {
        DesiredWidth::AtLeast(0.0)
    }

    /// Only required for hierarchical items?
    fn id(&self) -> egui::Id {
        egui::Id::NULL
    }
}

pub struct CustomListItemContent {
    ui: Box<dyn FnOnce(&crate::ReUi, &mut egui::Ui, &ContentContext<'_>)>,
}

impl CustomListItemContent {
    pub fn new(
        ui: impl FnOnce(&crate::ReUi, &mut egui::Ui, &ContentContext<'_>) + 'static,
    ) -> Self {
        Self { ui: Box::new(ui) }
    }
}

impl ListItemContent for CustomListItemContent {
    fn ui(
        mut self: Box<Self>,
        re_ui: &crate::ReUi,
        ui: &mut egui::Ui,
        context: &ContentContext<'_>,
    ) {
        (self.ui)(re_ui, ui, context)
    }
}
