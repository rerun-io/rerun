use egui::{Align, Align2, NumExt as _, RichText, Ui, text::TextWrapping};
use std::sync::Arc;

use super::{ContentContext, DesiredWidth, ListItemContent, ListVisuals};
use crate::{DesignTokens, Icon, LabelStyle};

/// [`ListItemContent`] that displays a simple label with optional icon and buttons.
#[allow(clippy::type_complexity)]
pub struct LabelContent<'a> {
    text: egui::WidgetText,

    //TODO(ab): these should probably go as WidgetText already implements that
    subdued: bool,
    weak: bool,
    italics: bool,

    label_style: LabelStyle,
    icon_fn: Option<Box<dyn FnOnce(&mut egui::Ui, egui::Rect, ListVisuals) + 'a>>,
    buttons_fn: Option<Box<dyn FnOnce(&mut egui::Ui) -> egui::Response + 'a>>,
    always_show_buttons: bool,

    text_wrap_mode: Option<egui::TextWrapMode>,
    min_desired_width: Option<f32>,
}

impl<'a> LabelContent<'a> {
    pub fn new(text: impl Into<egui::WidgetText>) -> Self {
        Self {
            text: text.into(),

            subdued: false,
            weak: false,
            italics: false,

            label_style: Default::default(),
            icon_fn: None,
            buttons_fn: None,
            always_show_buttons: false,

            text_wrap_mode: None,
            min_desired_width: None,
        }
    }

    /// Render this as a header item.
    ///
    /// Text will be strong and smaller.
    /// For best results, use this with [`super::ListItem::header`].
    pub fn header(text: impl Into<RichText>) -> Self {
        Self::new(
            text.into()
                .size(DesignTokens::list_header_font_size())
                .strong(),
        )
    }

    /// Set the subdued state of the item.
    ///
    /// Note: takes precedence over [`Self::weak`] if set.
    // TODO(ab): this is a hack to implement the behavior of the blueprint tree UI, where active
    // widget are displayed in a subdued state (container, hidden views/entities). One
    // slightly more correct way would be to override the color using a (color, index) pair
    // related to the design system table.
    #[inline]
    pub fn subdued(mut self, subdued: bool) -> Self {
        self.subdued = subdued;
        self
    }

    /// Set the weak state of the item.
    ///
    /// Note: [`Self::subdued`] takes precedence if set.
    // TODO(ab): should use design token instead
    #[inline]
    pub fn weak(mut self, weak: bool) -> Self {
        self.weak = weak;
        self
    }

    /// Render text in italic.
    // TODO(ab): should use design token instead
    #[inline]
    pub fn italics(mut self, italics: bool) -> Self {
        self.italics = italics;
        self
    }

    /// Style the label for an unnamed items.
    ///
    /// The styling is applied on top of to [`Self::weak`] and [`Self::subdued`]. It also implies [`Self::italics`].
    // TODO(ab): should use design token instead
    #[inline]
    pub fn label_style(mut self, style: crate::LabelStyle) -> Self {
        self.label_style = style;
        self
    }

    /// Should we truncate text if it is too long?
    #[inline]
    pub fn truncate(mut self, truncate: bool) -> Self {
        self.text_wrap_mode = Some(if truncate {
            egui::TextWrapMode::Truncate
        } else {
            egui::TextWrapMode::Extend
        });
        self
    }

    /// Set the minimum desired for the content.
    ///
    /// This defaults to zero.
    #[inline]
    pub fn min_desired_width(mut self, min_desired_width: f32) -> Self {
        self.min_desired_width = Some(min_desired_width);
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the item.
    #[inline]
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|ui, rect, visuals| {
            icon.as_image().tint(visuals.icon_tint()).paint_at(ui, rect);
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    #[inline]
    pub fn with_icon_fn(
        mut self,
        icon_fn: impl FnOnce(&mut egui::Ui, egui::Rect, ListVisuals) + 'a,
    ) -> Self {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Provide a closure to display on-hover buttons on the right of the item.
    ///
    /// Buttons also show when the item is selected, in order to support clicking them on touch
    /// screens. The buttons can be set to be always shown with [`Self::always_show_buttons`].
    ///
    /// If there are multiple buttons, the response returned should be the union of both buttons.
    ///
    /// Notes:
    /// - If buttons are used, the item will allocate the full available width of the parent. If the
    ///   enclosing UI adapts to the childrens width, it will unnecessarily grow. If buttons aren't
    ///   used, the item will only allocate the width needed for the text and icons if any.
    /// - A right to left layout is used, so the right-most button must be added first.
    // TODO(#6191): This should reconciled this with the `ItemButton` abstraction by using something
    //              like `Vec<Box<dyn ItemButton>>` instead of a generic closure.
    #[inline]
    pub fn with_buttons(
        mut self,
        buttons: impl FnOnce(&mut egui::Ui) -> egui::Response + 'a,
    ) -> Self {
        self.buttons_fn = Some(Box::new(buttons));
        self
    }

    /// Always show the buttons.
    ///
    /// By default, buttons are only shown when the item is hovered or selected. By setting this to
    /// `true`, the buttons are always shown.
    #[inline]
    pub fn always_show_buttons(mut self, always_show_buttons: bool) -> Self {
        self.always_show_buttons = always_show_buttons;
        self
    }

    fn get_text_wrap_mode(&self, ui: &egui::Ui) -> egui::TextWrapMode {
        if let Some(text_wrap_mode) = self.text_wrap_mode {
            text_wrap_mode
        } else if crate::is_in_resizable_panel(ui) {
            egui::TextWrapMode::Truncate // The user can resize the panl to see the full text
        } else {
            egui::TextWrapMode::Extend // Show everything
        }
    }
}

impl ListItemContent for LabelContent<'_> {
    fn ui(self: Box<Self>, ui: &mut Ui, context: &ContentContext<'_>) {
        let text_wrap_mode = self.get_text_wrap_mode(ui);

        let Self {
            mut text,
            subdued,
            weak,
            italics,
            label_style,
            icon_fn,
            buttons_fn,
            always_show_buttons,
            text_wrap_mode: _,
            min_desired_width: _,
        } = *self;

        let icon_rect = egui::Rect::from_center_size(
            context.rect.left_center() + egui::vec2(DesignTokens::small_icon_size().x / 2., 0.0),
            DesignTokens::small_icon_size(),
        );

        let mut text_rect = context.rect;
        if icon_fn.is_some() {
            text_rect.min.x += icon_rect.width() + DesignTokens::text_to_icon_padding();
        }

        // text styling
        if italics || label_style == LabelStyle::Unnamed {
            text = text.italics();
        }

        let visuals = context.visuals;

        let mut text_color = visuals.text_color();

        if weak {
            text_color = ui.style().visuals.gray_out(text_color);
        } else if subdued {
            text_color = text_color.gamma_multiply(0.5);
        }

        // Draw icon
        if let Some(icon_fn) = icon_fn {
            icon_fn(ui, icon_rect, visuals);
        }

        // We can't use `.hovered()` or the buttons disappear just as the user clicks,
        // so we use `contains_pointer` instead. That also means we need to check
        // that we aren't dragging anything.
        // By showing the buttons when selected, we allow users to find them on touch screens.
        let should_show_buttons = (context.list_item.interactive
            && ui.rect_contains_pointer(context.bg_rect)
            && !egui::DragAndDrop::has_any_payload(ui.ctx()))
            || context.list_item.selected
            || always_show_buttons;
        let button_response = if should_show_buttons {
            if let Some(buttons) = buttons_fn {
                let mut ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(text_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                );
                Some(buttons(&mut ui))
            } else {
                None
            }
        } else {
            None
        };

        // Draw text

        if let Some(button_response) = &button_response {
            text_rect.max.x -= button_response.rect.width() + DesignTokens::text_to_icon_padding();
        }

        let mut layout_job = Arc::unwrap_or_clone(text.into_layout_job(
            ui.style(),
            egui::FontSelection::Default,
            Align::LEFT,
        ));
        layout_job.wrap = TextWrapping::from_wrap_mode_and_width(text_wrap_mode, text_rect.width());

        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

        // this happens here to avoid cloning the text
        context.response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                context.list_item.selected,
                galley.text(),
            )
        });

        let text_pos = Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), text_rect)
            .min;

        ui.painter().galley(text_pos, galley, text_color);
    }

    fn desired_width(&self, ui: &Ui) -> DesiredWidth {
        let text_wrap_mode = self.get_text_wrap_mode(ui);

        let measured_width = {
            //TODO(ab): ideally there wouldn't be as much code duplication with `Self::ui`
            let mut text = self.text.clone();
            if self.italics || self.label_style == LabelStyle::Unnamed {
                text = text.italics();
            }

            let layout_job = Arc::unwrap_or_clone(text.clone().into_layout_job(
                ui.style(),
                egui::FontSelection::Default,
                Align::LEFT,
            ));
            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            let mut desired_width = galley.size().x;

            if self.icon_fn.is_some() {
                desired_width +=
                    DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding();
            }

            // The `ceil()` is needed to avoid some rounding errors which leads to text being
            // truncated even though we allocated enough space.
            desired_width.ceil()
        };

        if text_wrap_mode == egui::TextWrapMode::Extend {
            let min_desired_width = self.min_desired_width.unwrap_or(0.0);
            DesiredWidth::Exact(measured_width.at_least(min_desired_width))
        } else {
            // If the user set an explicit min-width, use it.
            // Otherwise, show at least `default_min_width`, unless the text is even short.
            let default_min_width = 64.0;
            let min_desired_width = self
                .min_desired_width
                .unwrap_or_else(|| measured_width.min(default_min_width));
            DesiredWidth::AtLeast(min_desired_width)
        }
    }
}
