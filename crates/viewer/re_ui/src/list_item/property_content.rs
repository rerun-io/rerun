use std::fmt::Display;
use std::sync::Arc;

use egui::text::TextWrapping;
use egui::{Align, Align2, NumExt as _, Ui};

use super::{
    ContentContext, DesiredWidth, ItemButtons, LayoutInfoStack, ListItemContent,
    ListItemContentButtonsExt, ListVisuals,
};
use crate::{Icon, UiExt as _};

/// Closure to draw an icon left of the label.
type IconFn<'a> = dyn FnOnce(&mut egui::Ui, egui::Rect, ListVisuals) + 'a;

/// Closure to draw the right column of the property.
type PropertyValueFn<'a> = dyn FnOnce(&mut egui::Ui, ListVisuals) + 'a;

/// [`ListItemContent`] to display property-like, two-column content.
///
/// The left column contains a label (along with an optional icon)
/// and the right column containing some custom value (which may be editable).
pub struct PropertyContent<'a> {
    label: egui::WidgetText,
    min_desired_width: f32,

    icon_fn: Option<Box<IconFn<'a>>>,
    show_only_when_collapsed: bool,
    value_fn: Option<Box<PropertyValueFn<'a>>>,
    //TODO(ab): in the future, that should be a `Vec`, with some auto expanding mini-toolbar
    buttons: ItemButtons<'a>,
    /**/
    //TODO(ab): icon styling? link icon right of label? clickable label?
}

impl<'a> PropertyContent<'a> {
    /// Spacing used between the two main columns
    const COLUMN_SPACING: f32 = 12.0;

    pub fn new(label: impl Into<egui::WidgetText>) -> Self {
        Self {
            label: label.into(),
            min_desired_width: 200.0,
            icon_fn: None,
            show_only_when_collapsed: true,
            value_fn: None,
            buttons: ItemButtons::default(),
        }
    }

    /// Set the minimum desired width for the entire content.
    ///
    /// Since there is no possible way to meaningfully collapse two to three columns worth of
    /// content, this is set to 200.0 by default.
    #[inline]
    pub fn min_desired_width(mut self, min_desired_width: f32) -> Self {
        self.min_desired_width = min_desired_width;
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the label.
    #[inline]
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|ui, rect, visuals| {
            icon.as_image().tint(visuals.icon_tint()).paint_at(ui, rect);
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    #[inline]
    pub fn with_icon_fn<F>(mut self, icon_fn: F) -> Self
    where
        F: FnOnce(&mut egui::Ui, egui::Rect, ListVisuals) + 'a,
    {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Display value only for leaf or collapsed items.
    ///
    /// When enabled, the value for this item is not displayed for uncollapsed hierarchical items.
    /// This is convenient when the value serves are a summary of the child content, which doesn't
    /// need to be displayed when said content is visible.
    ///
    /// Enabled by default.
    #[inline]
    pub fn show_only_when_collapsed(mut self, show_only_when_collapsed: bool) -> Self {
        self.show_only_when_collapsed = show_only_when_collapsed;
        self
    }

    /// Provide a closure to draw the content of the right column.
    #[inline]
    pub fn value_fn<F>(mut self, value_fn: F) -> Self
    where
        F: FnOnce(&mut egui::Ui, ListVisuals) + 'a,
    {
        self.value_fn = Some(Box::new(value_fn));
        self
    }

    //
    // Bunch of helpers with concrete implementation of value fn
    //

    /// Show a read-only boolean in the value column.
    #[inline]
    pub fn value_bool(self, mut b: bool) -> Self {
        if true {
            self.value_text(b.to_string())
        } else {
            // This is not readable, which is why it is disabled
            self.value_fn(move |ui: &mut Ui, _| {
                ui.add_enabled_ui(false, |ui| ui.re_checkbox(&mut b, ""));
            })
        }
    }

    /// Show an editable boolean in the value column.
    #[inline]
    pub fn value_bool_mut(self, b: &'a mut bool) -> Self {
        self.value_fn(|ui: &mut Ui, _| {
            ui.re_checkbox(b, "");
        })
    }

    /// Show a static text in the value column.
    #[inline]
    pub fn value_text(self, text: impl Into<egui::WidgetText> + 'a) -> Self {
        self.value_fn(move |ui, _| {
            ui.add(egui::Label::new(text.into()).truncate());
        })
    }

    /// Show a number, nicely formatted.
    #[inline]
    pub fn value_uint<Uint>(self, number: Uint) -> Self
    where
        Uint: Display + num_traits::Unsigned,
    {
        self.value_text(re_format::format_uint(number))
    }

    /// Show an editable text in the value column.
    #[inline]
    pub fn value_text_mut(self, text: &'a mut String) -> Self {
        self.value_fn(|ui, _| {
            ui.text_edit_singleline(text);
        })
    }

    /// Show a read-only color in the value column.
    #[inline]
    pub fn value_color(self, rgba: &'a [u8; 4]) -> Self {
        self.value_fn(|ui, _| {
            let [r, g, b, a] = rgba;
            #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
            let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
            let response = egui::color_picker::show_color(ui, color, ui.spacing().interact_size);
            response.on_hover_text(format!("Color #{r:02x}{g:02x}{b:02x}{a:02x}"));
        })
    }

    /// Show an editable color in the value column.
    #[inline]
    pub fn value_color_mut(self, rgba: &'a mut [u8; 4]) -> Self {
        self.value_fn(|ui: &mut egui::Ui, _| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.color_edit_button_srgba_unmultiplied(rgba);
        })
    }
}

impl ListItemContent for PropertyContent<'_> {
    fn ui(self: Box<Self>, ui: &mut Ui, context: &ContentContext<'_>) {
        ui.sanity_check();

        let Self {
            label,
            min_desired_width: _,
            icon_fn,
            show_only_when_collapsed,
            value_fn,
            buttons,
        } = *self;

        let tokens = ui.tokens();

        // │                                                                              │
        // │◀─────────────────────────────get_full_span()────────────────────────────────▶│
        // │                                                                              │
        // │ ◀────────layout_info.left_column_width─────────▶│┌──COLUMN_SPACING           │
        // │                                                  ▼                           │
        // │                       ◀─────────────────────────┼────────context.rect──────▶ │
        // │ ┌ ─ ─ ─ ─ ┬ ─ ─ ─ ─ ┬ ┬────────┬─┬─────────────┬─┬─────────────┬─┬─────────┐ │
        // │                       │        │ │             │││             │ │         │ │
        // │ │         │         │ │        │ │             │ │             │ │         │ │
        // │   INDENT       ▼      │  ICON  │ │    LABEL    │││    VALUE    │ │   BTN   │ │
        // │ │         │         │ │        │ │             │ │             │ │         │ │
        // │                       │        │ │             │││             │ │         │ │
        // │ └ ─ ─ ─ ─ ┴ ─ ─ ─ ─ ┴ ┴────────┴─┴─────────────┴─┴─────────────┴─┴─────────┘ │
        // │ ▲                     ▲         ▲               │               ▲            │
        // │ └──layout_info.left_x │         └───────────────────────────────┤            │
        // │                       │                         ▲               │            │
        // │       content_left_x──┘           mid_point_x───┘    text_to_icon_padding()  │
        // │                                                                              │

        let content_left_x = context.rect.left();
        // Total indent left of the content rect. This is part of the left column width.
        let content_indent = content_left_x - context.layout_info.left_x;
        let mid_point_x = context.layout_info.left_x
            + context
                .layout_info
                .left_column_width
                .unwrap_or_else(|| content_indent + (context.rect.width() / 2.).at_least(0.0));

        let icon_extra = if icon_fn.is_some() {
            tokens.small_icon_size.x + tokens.text_to_icon_padding()
        } else {
            0.0
        };

        let label_rect = egui::Rect::from_x_y_ranges(
            (content_left_x + icon_extra)..=(mid_point_x - Self::COLUMN_SPACING / 2.0),
            context.rect.y_range(),
        );

        let mut value_rect = egui::Rect::from_x_y_ranges(
            (mid_point_x + Self::COLUMN_SPACING / 2.0)..=context.rect.right(),
            context.rect.y_range(),
        );

        buttons.show_and_shrink_rect(ui, context, &mut value_rect);

        let visuals = context.visuals;

        // Draw icon
        if let Some(icon_fn) = icon_fn {
            let icon_rect = egui::Rect::from_center_size(
                context.rect.left_center() + egui::vec2(tokens.small_icon_size.x / 2., 0.0),
                tokens.small_icon_size,
            );

            icon_fn(ui, icon_rect, visuals);
        }

        // Prepare the label galley. We first go for an un-truncated version to register our desired
        // column width. If it doesn't fit the available space, we recreate it with truncation.
        let mut layout_job = Arc::unwrap_or_clone(label.into_layout_job(
            ui.style(),
            egui::FontSelection::Default,
            Align::LEFT,
        ));
        let desired_galley = ui.fonts_mut(|fonts| fonts.layout_job(layout_job.clone()));
        let desired_width =
            (content_indent + icon_extra + desired_galley.size().x + Self::COLUMN_SPACING / 2.0)
                .ceil();

        context
            .layout_info
            .register_desired_left_column_width(ui, desired_width);

        let galley = if desired_galley.size().x <= label_rect.width() {
            desired_galley
        } else {
            layout_job.wrap = TextWrapping::truncate_at_width(label_rect.width());
            ui.fonts_mut(|fonts| fonts.layout_job(layout_job))
        };

        // this happens here to avoid cloning the text
        context.response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                context.list_item.selected,
                galley.text(),
            )
        });

        // Label ready to draw.
        let text_pos = Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), label_rect)
            .min;
        let mut visuals_for_label = visuals;
        visuals_for_label.interactive = false;
        ui.painter()
            .galley(text_pos, galley, visuals_for_label.text_color());

        let mut visuals_for_value = visuals;
        visuals_for_value.strong = true;
        visuals_for_value.interactive = true; // interactive false would override strong

        // Draw value
        let is_completely_collapsed = context.list_item.collapse_openness.is_none_or(|o| o == 0.0);
        let should_show_value = if show_only_when_collapsed {
            is_completely_collapsed
        } else {
            true
        };
        if let Some(value_fn) = value_fn
            && should_show_value
        {
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(value_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            // This sets the default text color for e.g. ui.label, but syntax highlighted
            // text won't be overridden
            child_ui
                .visuals_mut()
                .widgets
                .noninteractive
                .fg_stroke
                .color = visuals_for_value.text_color();
            // When selected we override the text color so e.g. syntax highlighted code
            // doesn't become unreadable
            if context.visuals.selected {
                child_ui.visuals_mut().override_text_color = Some(visuals_for_value.text_color());
            }

            child_ui.sanity_check();
            value_fn(&mut child_ui, visuals_for_value);
            child_ui.sanity_check();

            context.layout_info.register_property_content_max_width(
                &child_ui,
                child_ui.min_rect().right() - context.layout_info.left_x,
            );
        }
    }

    fn desired_width(&self, ui: &Ui) -> DesiredWidth {
        ui.sanity_check();

        let layout_info = LayoutInfoStack::top(ui.ctx());

        if crate::is_in_resizable_panel(ui) {
            DesiredWidth::AtLeast(self.min_desired_width)
        } else if let Some(max_width) = layout_info.property_content_max_width {
            let desired_width = max_width + layout_info.left_x - ui.max_rect().left();

            DesiredWidth::AtLeast(desired_width.ceil())
        } else {
            DesiredWidth::AtLeast(self.min_desired_width)
        }
    }
}

impl<'a> ListItemContentButtonsExt<'a> for PropertyContent<'a> {
    fn buttons(&self) -> &ItemButtons<'a> {
        &self.buttons
    }

    fn buttons_mut(&mut self) -> &mut ItemButtons<'a> {
        &mut self.buttons
    }
}
