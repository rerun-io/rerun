mod container;
mod debug;
mod list_item;

pub use container::*;
pub use debug::*;
pub use list_item::*;

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

    pub state: &'a State,
    //TODO: provide a way to affect ListItem background, e.g. current button hover behaviour
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
    fn ui(&mut self, re_ui: &crate::ReUi, ui: &egui::Ui, context: &ContentContext<'_>);

    fn desired_width(&self, _re_ui: &crate::ReUi, _ui: &egui::Ui) -> DesiredWidth {
        DesiredWidth::AtLeast(0.0)
    }

    /// Only required for hierarchical items?
    fn id(&self) -> egui::Id {
        egui::Id::NULL
    }
}

pub struct CustomListItemContent {
    ui: Box<dyn FnMut(&crate::ReUi, &egui::Ui, &ContentContext<'_>)>,
}

impl CustomListItemContent {
    pub fn new(ui: impl FnMut(&crate::ReUi, &egui::Ui, &ContentContext<'_>) + 'static) -> Self {
        Self { ui: Box::new(ui) }
    }
}

impl ListItemContent for CustomListItemContent {
    fn ui(&mut self, re_ui: &crate::ReUi, ui: &egui::Ui, context: &ContentContext<'_>) {
        (self.ui)(re_ui, ui, context)
    }
}
