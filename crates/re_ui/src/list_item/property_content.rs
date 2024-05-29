use egui::{text::TextWrapping, Align, Align2, NumExt as _, Ui};

use super::{ContentContext, DesiredWidth, LayoutInfoStack, ListItemContent};
use crate::{Icon, ReUi};

/// Closure to draw an icon left of the label.
type IconFn<'a> = dyn FnOnce(&ReUi, &mut egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a;

/// Closure to draw the right column of the property.
type PropertyValueFn<'a> = dyn FnOnce(&ReUi, &mut egui::Ui, egui::style::WidgetVisuals) + 'a;

struct PropertyActionButton<'a> {
    icon: &'static crate::icons::Icon,
    enabled: bool,
    on_click: Box<dyn FnOnce() + 'a>,
}

/// [`ListItemContent`] to display property-like, two-column content, with the left column
/// containing a label (along with an optional icon) and the right column containing some custom
/// value (which may be editable).
pub struct PropertyContent<'a> {
    label: egui::WidgetText,
    min_desired_width: f32,
    exact_width: bool,

    icon_fn: Option<Box<IconFn<'a>>>,
    show_only_when_collapsed: bool,
    value_fn: Option<Box<PropertyValueFn<'a>>>,
    //TODO(ab): in the future, that should be a `Vec`, with some auto expanding mini-toolbar
    action_buttons: Option<PropertyActionButton<'a>>,
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
            exact_width: false,
            icon_fn: None,
            show_only_when_collapsed: true,
            value_fn: None,
            action_buttons: None,
        }
    }

    /// Set the minimum desired width for the entire content.
    ///
    /// Since there is no possibly way to meaningfully collapse two to three columns worth of
    /// content, this is set to 200.0 by default.
    #[inline]
    pub fn min_desired_width(mut self, min_desired_width: f32) -> Self {
        self.min_desired_width = min_desired_width;
        self
    }

    /// Allocate the exact width required for the entire content.
    ///
    /// Note: this is done by tracking the maximum width in the current [`super::list_item_scope`]
    /// during the previous frame, so this is effective on the second frame only. If the first frame
    /// is actually rendered, this can lead to a flicker.
    #[inline]
    pub fn exact_width(mut self, exact_width: bool) -> Self {
        self.exact_width = exact_width;
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the label.
    #[inline]
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|_, ui, rect, visuals| {
            let tint = visuals.fg_stroke.color;
            icon.as_image().tint(tint).paint_at(ui, rect);
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    #[inline]
    pub fn with_icon_fn<F>(mut self, icon_fn: F) -> Self
    where
        F: FnOnce(&ReUi, &mut egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a,
    {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Right aligned action button.
    ///
    /// Note: for aesthetics, space is always reserved for the action button.
    // TODO(#6191): accept multiple calls for this function for multiple actions.
    #[inline]
    pub fn action_button(
        self,
        icon: &'static crate::icons::Icon,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        self.action_button_with_enabled(icon, true, on_click)
    }

    /// Right aligned action button.
    ///
    /// Note: for aesthetics, space is always reserved for the action button.
    // TODO(#6191): accept multiple calls for this function for multiple actions.
    #[inline]
    pub fn action_button_with_enabled(
        mut self,
        icon: &'static crate::icons::Icon,
        enabled: bool,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        // TODO(#6191): support multiple action buttons
        assert!(
            self.action_buttons.is_none(),
            "Only one action button supported right now"
        );
        self.action_buttons = Some(PropertyActionButton {
            icon,
            enabled,
            on_click: Box::new(on_click),
        });
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
        F: FnOnce(&ReUi, &mut egui::Ui, egui::style::WidgetVisuals) + 'a,
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
        self.value_fn(move |_, ui: &mut Ui, _| {
            ui.add_enabled(false, crate::toggle_switch(15.0, &mut b));
        })
    }

    /// Show an editable boolean in the value column.
    #[inline]
    pub fn value_bool_mut(self, b: &'a mut bool) -> Self {
        self.value_fn(|_, ui: &mut Ui, _| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;

            ui.add(crate::toggle_switch(15.0, b));
        })
    }

    /// Show a static text in the value column.
    #[inline]
    pub fn value_text(self, text: impl Into<egui::WidgetText> + 'a) -> Self {
        self.value_fn(move |_, ui, _| {
            ui.add(egui::Label::new(text.into()).truncate());
        })
    }

    /// Show an editable text in the value column.
    #[inline]
    pub fn value_text_mut(self, text: &'a mut String) -> Self {
        self.value_fn(|_, ui, _| {
            ui.text_edit_singleline(text);
        })
    }

    /// Show a read-only color in the value column.
    #[inline]
    pub fn value_color(self, rgba: &'a [u8; 4]) -> Self {
        self.value_fn(|_, ui, _| {
            let [r, g, b, a] = rgba;
            let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
            let response = egui::color_picker::show_color(ui, color, ui.spacing().interact_size);
            response.on_hover_text(format!("Color #{r:02x}{g:02x}{b:02x}{a:02x}"));
        })
    }

    /// Show an editable color in the value column.
    #[inline]
    pub fn value_color_mut(self, rgba: &'a mut [u8; 4]) -> Self {
        self.value_fn(|_, ui: &mut egui::Ui, _| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.color_edit_button_srgba_unmultiplied(rgba);
        })
    }
}

impl ListItemContent for PropertyContent<'_> {
    fn ui(self: Box<Self>, re_ui: &ReUi, ui: &mut Ui, context: &ContentContext<'_>) {
        let Self {
            label,
            min_desired_width: _,
            icon_fn,
            show_only_when_collapsed,
            value_fn,
            action_buttons,
            exact_width: _,
        } = *self;

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
            ReUi::small_icon_size().x + ReUi::text_to_icon_padding()
        } else {
            0.0
        };

        // Based on egui::ImageButton::ui()
        let action_button_dimension =
            ReUi::small_icon_size().x + 2.0 * ui.spacing().button_padding.x;
        let reserve_action_button_space =
            action_buttons.is_some() || context.layout_info.reserve_action_button_space;
        let action_button_extra = if reserve_action_button_space {
            action_button_dimension + ReUi::text_to_icon_padding()
        } else {
            0.0
        };

        let label_rect = egui::Rect::from_x_y_ranges(
            (content_left_x + icon_extra)..=(mid_point_x - Self::COLUMN_SPACING / 2.0),
            context.rect.y_range(),
        );

        let value_rect = egui::Rect::from_x_y_ranges(
            (mid_point_x + Self::COLUMN_SPACING / 2.0)
                ..=(context.rect.right() - action_button_extra),
            context.rect.y_range(),
        );

        let visuals = ui
            .style()
            .interact_selectable(context.response, context.list_item.selected);

        // Draw icon
        if let Some(icon_fn) = icon_fn {
            let icon_rect = egui::Rect::from_center_size(
                context.rect.left_center() + egui::vec2(ReUi::small_icon_size().x / 2., 0.0),
                ReUi::small_icon_size(),
            );

            icon_fn(re_ui, ui, icon_rect, visuals);
        }

        // Prepare the label galley. We first go for an un-truncated version to register our desired
        // column width. If it doesn't fit the available space, we recreate it with truncation.
        let mut layout_job =
            label.into_layout_job(ui.style(), egui::FontSelection::Default, Align::LEFT);
        let desired_galley = ui.fonts(|fonts| fonts.layout_job(layout_job.clone()));
        let desired_width =
            (content_indent + icon_extra + desired_galley.size().x + Self::COLUMN_SPACING / 2.0)
                .ceil();

        context
            .layout_info
            .register_desired_left_column_width(ui.ctx(), desired_width);
        context
            .layout_info
            .reserve_action_button_space(ui.ctx(), action_buttons.is_some());

        let galley = if desired_galley.size().x <= label_rect.width() {
            desired_galley
        } else {
            layout_job.wrap = TextWrapping::truncate_at_width(label_rect.width());
            ui.fonts(|fonts| fonts.layout_job(layout_job))
        };

        // this happens here to avoid cloning the text
        context.response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                context.list_item.selected,
                galley.text(),
            )
        });

        // Label ready to draw.
        let text_pos = Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), label_rect)
            .min;
        ui.painter().galley(text_pos, galley, visuals.text_color());

        // Draw value
        let is_completely_collapsed = context
            .list_item
            .collapse_openness
            .map_or(true, |o| o == 0.0);
        let should_show_value = if show_only_when_collapsed {
            is_completely_collapsed
        } else {
            true
        };
        if let Some(value_fn) = value_fn {
            if should_show_value {
                let mut child_ui =
                    ui.child_ui(value_rect, egui::Layout::left_to_right(egui::Align::Center));
                value_fn(re_ui, &mut child_ui, visuals);

                context.layout_info.register_property_content_max_width(
                    child_ui.ctx(),
                    child_ui.min_rect().right() - context.layout_info.left_x,
                );
            }
        }

        // Draw action button
        if let Some(action_button) = action_buttons {
            let action_button_rect = egui::Rect::from_center_size(
                context.rect.right_center() - egui::vec2(action_button_dimension / 2.0, 0.0),
                egui::Vec2::splat(action_button_dimension),
            );

            // the right to left layout is used to mimic LabelContent's buttons behavior and get a
            // better alignment
            let mut child_ui = ui.child_ui(
                action_button_rect,
                egui::Layout::right_to_left(egui::Align::Center),
            );

            child_ui.add_enabled_ui(action_button.enabled, |ui| {
                let button_response = re_ui.small_icon_button(ui, action_button.icon);
                if button_response.clicked() {
                    (action_button.on_click)();
                }
            });
        }
    }

    fn desired_width(&self, _re_ui: &ReUi, ui: &Ui) -> DesiredWidth {
        let layout_info = LayoutInfoStack::top(ui.ctx());
        if self.exact_width {
            if let Some(max_width) = layout_info.property_content_max_width {
                let mut desired_width = max_width + layout_info.left_x - ui.max_rect().left();

                // TODO(ab): ideally there wouldn't be as much code duplication with `Self::ui`
                let action_button_dimension =
                    ReUi::small_icon_size().x + 2.0 * ui.spacing().button_padding.x;
                let reserve_action_button_space =
                    self.action_buttons.is_some() || layout_info.reserve_action_button_space;
                if reserve_action_button_space {
                    desired_width += action_button_dimension + ReUi::text_to_icon_padding();
                }

                DesiredWidth::Exact(desired_width.ceil())
            } else {
                DesiredWidth::AtLeast(self.min_desired_width)
            }
        } else {
            DesiredWidth::AtLeast(self.min_desired_width)
        }
    }
}
