use crate::list_item2::{ContentContext, DesiredWidth, ListItemContent};
use crate::{Icon, LabelStyle, ReUi};
use eframe::emath::{Align, Align2};
use eframe::epaint::text::TextWrapping;
use egui::Ui;

/// [`ListItemContent`] that displays a simple label with optional icon and buttons.
#[allow(clippy::type_complexity)]
pub struct LabelContent<'a> {
    text: egui::WidgetText,

    //TODO(ab): these should probably go as WidgetText already implements that
    subdued: bool,
    weak: bool,
    italics: bool,

    label_style: LabelStyle,
    icon_fn: Option<Box<dyn FnOnce(&ReUi, &egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a>>,
    buttons_fn: Option<Box<dyn FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a>>,

    exact_width: bool,
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
            exact_width: false,
        }
    }

    /// Set the subdued state of the item.
    ///
    /// Note: takes precedence over [`Self::weak`] if set.
    // TODO(ab): this is a hack to implement the behavior of the blueprint tree UI, where active
    // widget are displayed in a subdued state (container, hidden space views/entities). One
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

    /// Allocate the exact width required for the label.
    ///
    /// By default, [`LabelContent`] uses the available width. By setting `exact_width` to true,
    /// the exact width required by the label (and the icon if any) is allocated instead. See
    /// [`super::DesiredWidth::Exact`].
    #[inline]
    pub fn exact_width(mut self, exact_width: bool) -> Self {
        self.exact_width = exact_width;
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the item.
    #[inline]
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|_, ui, rect, visuals| {
            let tint = visuals.fg_stroke.color;
            icon.as_image().tint(tint).paint_at(ui, rect);
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    #[inline]
    pub fn with_icon_fn(
        mut self,
        icon_fn: impl FnOnce(&ReUi, &egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a,
    ) -> Self {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Provide a closure to display on-hover buttons on the right of the item.
    ///
    /// Buttons also show when the item is selected, in order to support clicking them on touch screens.
    ///
    /// Notes:
    /// - If buttons are used, the item will allocate the full available width of the parent. If the
    ///   enclosing UI adapts to the childrens width, it will unnecessarily grow. If buttons aren't
    ///   used, the item will only allocate the width needed for the text and icons if any.
    /// - A right to left layout is used, so the right-most button must be added first.
    #[inline]
    pub fn with_buttons(
        mut self,
        buttons: impl FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a,
    ) -> Self {
        self.buttons_fn = Some(Box::new(buttons));
        self
    }
}

impl ListItemContent for LabelContent<'_> {
    fn ui(self: Box<Self>, re_ui: &ReUi, ui: &mut Ui, context: &ContentContext<'_>) {
        let Self {
            mut text,
            subdued,
            weak,
            italics,
            label_style,
            icon_fn,
            buttons_fn,
            exact_width: _,
        } = *self;

        let icon_rect = egui::Rect::from_center_size(
            context.rect.left_center() + egui::vec2(ReUi::small_icon_size().x / 2., 0.0),
            ReUi::small_icon_size(),
        );

        let mut text_rect = context.rect;
        if icon_fn.is_some() {
            text_rect.min.x += icon_rect.width() + ReUi::text_to_icon_padding();
        }

        // text styling
        if italics || label_style == LabelStyle::Unnamed {
            text = text.italics();
        }

        let mut visuals = ui
            .style()
            .interact_selectable(context.response, context.list_item.selected);

        // TODO(ab): use design tokens instead
        if weak {
            visuals.fg_stroke.color = ui.visuals().weak_text_color();
        } else if subdued {
            visuals.fg_stroke.color = visuals.fg_stroke.color.gamma_multiply(0.5);
        }

        match label_style {
            LabelStyle::Normal => {}
            LabelStyle::Unnamed => {
                text = text.color(visuals.fg_stroke.color.gamma_multiply(0.5));
            }
        }

        // Draw icon
        if let Some(icon_fn) = icon_fn {
            icon_fn(re_ui, ui, icon_rect, visuals);
        }

        // We can't use `.hovered()` or the buttons disappear just as the user clicks,
        // so we use `contains_pointer` instead. That also means we need to check
        // that we aren't dragging anything.
        let should_show_buttons = context.list_item.interactive
            && ui.rect_contains_pointer(context.bg_rect)
            && !egui::DragAndDrop::has_any_payload(ui.ctx())
            || context.list_item.selected; // by showing the buttons when selected, we allow users to find them on touch screens
        let button_response = if should_show_buttons {
            if let Some(buttons) = buttons_fn {
                let mut ui =
                    ui.child_ui(text_rect, egui::Layout::right_to_left(egui::Align::Center));
                Some(buttons(re_ui, &mut ui))
            } else {
                None
            }
        } else {
            None
        };

        // Draw text

        if let Some(button_response) = &button_response {
            text_rect.max.x -= button_response.rect.width() + ReUi::text_to_icon_padding();
        }

        let mut layout_job =
            text.into_layout_job(ui.style(), egui::FontSelection::Default, Align::LEFT);
        layout_job.wrap = TextWrapping::truncate_at_width(text_rect.width());

        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

        // this happens here to avoid cloning the text
        context.response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                context.list_item.selected,
                galley.text(),
            )
        });

        let text_pos = Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), text_rect)
            .min;

        ui.painter().galley(text_pos, galley, visuals.text_color());
    }

    fn desired_width(&self, _re_ui: &ReUi, ui: &Ui) -> DesiredWidth {
        if self.exact_width {
            //TODO(ab): ideally there wouldn't be as much code duplication with `Self::ui`
            let mut text = self.text.clone();
            if self.italics || self.label_style == LabelStyle::Unnamed {
                text = text.italics();
            }

            let layout_job =
                text.clone()
                    .into_layout_job(ui.style(), egui::FontSelection::Default, Align::LEFT);
            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            let mut desired_width = galley.size().x;

            if self.icon_fn.is_some() {
                desired_width += ReUi::small_icon_size().x + ReUi::text_to_icon_padding();
            }

            // The `ceil()` is needed to avoid some rounding errors which leads to text being
            // truncated even though we allocated enough space.
            DesiredWidth::Exact(desired_width.ceil())
        } else {
            DesiredWidth::default()
        }
    }
}
