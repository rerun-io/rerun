use std::sync::Arc;

use crate::UiExt as _;

/// Specifies the context in which the UI is used and the constraints it should follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLayout {
    /// Display a short summary. Used in lists.
    ///
    /// Keep it small enough to fit on half a row (i.e. the second column of a
    /// [`crate::list_item::ListItem`] with [`crate::list_item::PropertyContent`]. Text should
    /// truncate.
    List,

    /// Display as much information as possible in a compact way. Used for hovering/tooltips.
    ///
    /// Keep it under a half-dozen lines. Text may wrap. Avoid interactive UI. When using a table,
    /// use the [`Self::table`] function.
    Tooltip,

    /// Display everything as wide as available, without height restriction. Used in the selection
    /// panel when a single item is selected.
    ///
    /// The UI will be wrapped in a [`egui::ScrollArea`], so data should be fully displayed with no
    /// restriction. When using a table, use the [`Self::table`] function.
    SelectionPanel,
}

impl UiLayout {
    /// Should the UI fit on one line?
    #[inline]
    pub fn is_single_line(&self) -> bool {
        match self {
            Self::List => true,
            Self::Tooltip | Self::SelectionPanel => false,
        }
    }

    /// Do we have a lot of vertical space?
    #[inline]
    pub fn is_selection_panel(self) -> bool {
        match self {
            Self::List | Self::Tooltip => false,
            Self::SelectionPanel => true,
        }
    }

    /// Build an egui table and configure it for the given UI layout.
    ///
    /// Note that the caller is responsible for strictly limiting the number of displayed rows for
    /// [`Self::List`] and [`Self::Tooltip`], as the table will not scroll.
    pub fn table(self, ui: &mut egui::Ui) -> egui_extras::TableBuilder<'_> {
        let table = egui_extras::TableBuilder::new(ui);
        match self {
            Self::List | Self::Tooltip => {
                // Be as small as possible in the hover tooltips. No scrolling related configuration, as
                // the content itself must be limited (scrolling is not possible in tooltips).
                table.auto_shrink([true, true])
            }
            Self::SelectionPanel => {
                // We're alone in the selection panel. Let the outer ScrollArea do the work.
                table.auto_shrink([false, true]).vscroll(false)
            }
        }
    }

    /// Show a label while respecting the given UI layout.
    ///
    /// Important: for label only, data should use [`UiLayout::data_label`] instead.
    // TODO(#6315): must be merged with `Self::data_label` and have an improved API
    pub fn label(self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
        let mut label = egui::Label::new(text);

        // Respect set wrap_mode if already set
        if ui.style().wrap_mode.is_none() {
            match self {
                Self::List => {
                    if ui.is_sizing_pass() {
                        // grow parent if needed - that's the point of a sizing pass
                        label = label.extend();
                    } else {
                        label = label.truncate();
                    }
                }
                Self::Tooltip | Self::SelectionPanel => {
                    label = label.wrap();
                }
            }
        }

        ui.add(label)
    }

    /// Show data while respecting the given UI layout.
    ///
    /// Import: for data only, labels should use [`UiLayout::label`] instead.
    // TODO(#6315): must be merged with `Self::label` and have an improved API
    pub fn data_label(self, ui: &mut egui::Ui, string: impl AsRef<str>) -> egui::Response {
        self.data_label_impl(ui, string.as_ref())
    }

    fn decorate_url(ui: &mut egui::Ui, text: &str, galley: Arc<egui::Galley>) -> egui::Response {
        // By default e.g., "droid:full" would be considered a valid URL. We decided we only care
        // about sane URL formats that include "://". This means e.g., "mailto:hello@world" won't
        // be considered a URL, but that is preferable to showing links for anything with a colon.
        if text.contains("://") && url::Url::parse(text).is_ok() {
            // This is a general link and should not open a new tab unless desired by the user.
            ui.re_hyperlink(text, text, false)
        } else {
            ui.label(galley)
        }
    }

    fn data_label_impl(self, ui: &mut egui::Ui, string: &str) -> egui::Response {
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let color = ui.visuals().text_color();
        let wrap_width = ui.available_width();
        let mut layout_job =
            egui::text::LayoutJob::simple(string.to_owned(), font_id, color, wrap_width);

        match self {
            Self::List => {
                layout_job.wrap.max_rows = 1; // We must fit on one line
                if ui.is_sizing_pass() {
                    // grow parent if needed - that's the point of a sizing pass
                    layout_job.wrap.max_width = f32::INFINITY;
                } else {
                    // Truncate
                    layout_job.wrap.break_anywhere = true;
                }
            }
            Self::Tooltip => {
                layout_job.wrap.max_rows = 3;
            }
            Self::SelectionPanel => {}
        }

        let galley = ui.fonts(|f| f.layout_job(layout_job)); // We control the text layout; not the label

        Self::decorate_url(ui, string, galley)
    }
}
