use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{NumExt as _, TextWrapMode};

use crate::UiExt as _;
use crate::syntax_highlighting::SyntaxHighlightedBuilder;

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
            let wrap_mode = match self {
                Self::List => {
                    if ui.is_sizing_pass() {
                        if ui.is_tooltip() {
                            TextWrapMode::Truncate // Dangerous to let this grow without bounds. TODO(emilk): let it grow up to `tooltip_width`
                        } else {
                            // grow parent if needed - that's the point of a sizing pass
                            TextWrapMode::Extend
                        }
                    } else {
                        TextWrapMode::Truncate
                    }
                }
                Self::Tooltip | Self::SelectionPanel => TextWrapMode::Wrap,
            };

            label = label.wrap_mode(wrap_mode);
        }

        ui.add(label)
    }

    /// Show data while respecting the given UI layout.
    ///
    /// Import: for data only, labels should use [`UiLayout::label`] instead.
    ///
    /// Make sure to use the right syntax highlighting. Check [`SyntaxHighlightedBuilder`] docs
    /// for details.
    // TODO(#6315): must be merged with `Self::label` and have an improved API
    pub fn data_label(
        self,
        ui: &mut egui::Ui,
        data: impl Into<SyntaxHighlightedBuilder>,
    ) -> egui::Response {
        self.data_label_impl(ui, data.into().into_job(ui.style()))
    }

    fn decorate_url(ui: &mut egui::Ui, mut galley: Arc<egui::Galley>) -> egui::Response {
        ui.sanity_check();

        if ui.layer_id().order == egui::Order::Tooltip
            && ui.spacing().tooltip_width < galley.size().x
        {
            // This will make the tooltip too wide.
            // TODO(#11211): do proper fix

            re_log::debug_assert!(
                galley.size().x < ui.spacing().tooltip_width + 1000.0,
                "adding huge galley with width: {} to a tooltip.",
                galley.size().x
            );

            // Ugly hack that may or may not work correctly.
            let mut layout_job = Arc::unwrap_or_clone(galley.job.clone());
            layout_job.wrap.max_width = ui.spacing().tooltip_width;
            galley = ui.fonts_mut(|f| f.layout_job(layout_job));
        }

        let text = galley.text();
        // By default e.g., "droid:full" would be considered a valid URL. We decided we only care
        // about sane URL formats that include "://". This means e.g., "mailto:hello@world" won't
        // be considered a URL, but that is preferable to showing links for anything with a colon.
        if text.contains("://") {
            // Syntax highlighting may add quotes around strings.
            let stripped = text.trim_matches(SyntaxHighlightedBuilder::QUOTE_CHAR);
            if url::Url::parse(stripped).is_ok() {
                // This is a general link and should not open a new tab unless desired by the user.
                return ui.re_hyperlink(galley.clone(), stripped, false);
            }
        }
        let response = ui.label(galley);
        ui.sanity_check();
        response
    }

    fn data_label_impl(self, ui: &mut egui::Ui, mut layout_job: LayoutJob) -> egui::Response {
        let wrap_width = ui.available_width();
        layout_job.wrap = TextWrapping::wrap_at_width(wrap_width);

        match self {
            Self::List => {
                layout_job.wrap.max_rows = 1; // We must fit on one line

                // Show the whole text; not just the first line.
                // See https://github.com/rerun-io/rerun/issues/10653
                // Ideally egui would allow us to configure what replacement character to use
                // instead of newline, but for now it doesn't, so `\n` will show up as a square.
                layout_job.break_on_newline = false;

                if ui.is_sizing_pass() {
                    if ui.is_tooltip() {
                        // We should only allow this to grow up to the width of the tooltip:
                        let max_tooltip_width = ui.style().spacing.tooltip_width;
                        let growth_margin = max_tooltip_width - ui.max_rect().width();

                        layout_job.wrap.max_width += growth_margin;

                        // There are limits to how small we shrink this though,
                        // even at the cost of making the tooltip too wide.
                        layout_job.wrap.max_width = layout_job.wrap.max_width.at_least(10.0);
                    } else {
                        // grow parent if needed - that's the point of a sizing pass
                        layout_job.wrap.max_width = f32::INFINITY;
                    }
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

        let galley = ui.fonts_mut(|f| f.layout_job(layout_job)); // We control the text layout; not the label

        Self::decorate_url(ui, galley)
    }
}
