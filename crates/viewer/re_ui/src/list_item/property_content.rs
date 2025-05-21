use egui::{Align, Align2, NumExt as _, Ui, text::TextWrapping};
use std::sync::Arc;

use super::{ContentContext, DesiredWidth, LayoutInfoStack, ListItemContent, ListVisuals};
use crate::{DesignTokens, Icon, UiExt as _};

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
    button: Option<Box<dyn super::ItemButton + 'a>>,
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
            button: None,
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

    /// Add a right-aligned [`super::ItemButton`].
    ///
    /// Note: for aesthetics, space is always reserved for the action button.
    // TODO(#6191): accept multiple calls for this function for multiple actions.
    #[inline]
    pub fn button(mut self, button: impl super::ItemButton + 'a) -> Self {
        // TODO(#6191): support multiple action buttons
        assert!(
            self.button.is_none(),
            "Only one action button is supported right now"
        );

        self.button = Some(Box::new(button));
        self
    }

    /// Helper to add an [`super::ItemActionButton`] to the right of the item.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn action_button(
        self,
        icon: &'static crate::icons::Icon,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        self.action_button_with_enabled(icon, true, on_click)
    }

    /// Helper to add an enabled/disabled [`super::ItemActionButton`] to the right of the item.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn action_button_with_enabled(
        self,
        icon: &'static crate::icons::Icon,
        enabled: bool,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        self.button(super::ItemActionButton::new(icon, on_click).enabled(enabled))
    }

    /// Helper to add a [`super::ItemMenuButton`] to the right of the item.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn menu_button(
        self,
        icon: &'static crate::icons::Icon,
        add_contents: impl FnOnce(&mut egui::Ui) + 'a,
    ) -> Self {
        self.button(super::ItemMenuButton::new(icon, add_contents))
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
            #[allow(clippy::disallowed_methods)] // This is not a hard-coded color.
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
        let Self {
            label,
            min_desired_width: _,
            icon_fn,
            show_only_when_collapsed,
            value_fn,
            button,
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
            DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding()
        } else {
            0.0
        };

        // Based on egui::ImageButton::ui()
        let action_button_dimension =
            DesignTokens::small_icon_size().x + 2.0 * ui.spacing().button_padding.x;
        let reserve_action_button_space =
            button.is_some() || context.layout_info.reserve_action_button_space;
        let action_button_extra = if reserve_action_button_space {
            action_button_dimension + DesignTokens::text_to_icon_padding()
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

        let visuals = context.visuals;

        // Draw icon
        if let Some(icon_fn) = icon_fn {
            let icon_rect = egui::Rect::from_center_size(
                context.rect.left_center()
                    + egui::vec2(DesignTokens::small_icon_size().x / 2., 0.0),
                DesignTokens::small_icon_size(),
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
        let desired_galley = ui.fonts(|fonts| fonts.layout_job(layout_job.clone()));
        let desired_width =
            (content_indent + icon_extra + desired_galley.size().x + Self::COLUMN_SPACING / 2.0)
                .ceil();

        context
            .layout_info
            .register_desired_left_column_width(ui.ctx(), desired_width);
        context
            .layout_info
            .reserve_action_button_space(ui.ctx(), button.is_some());

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
                ui.is_enabled(),
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
        let is_completely_collapsed = context.list_item.collapse_openness.is_none_or(|o| o == 0.0);
        let should_show_value = if show_only_when_collapsed {
            is_completely_collapsed
        } else {
            true
        };
        if let Some(value_fn) = value_fn {
            if should_show_value {
                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(value_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                value_fn(&mut child_ui, visuals);

                context.layout_info.register_property_content_max_width(
                    child_ui.ctx(),
                    child_ui.min_rect().right() - context.layout_info.left_x,
                );
            }
        }

        // Draw action button
        if let Some(button) = button {
            let action_button_rect = egui::Rect::from_center_size(
                context.rect.right_center() - egui::vec2(action_button_dimension / 2.0, 0.0),
                egui::Vec2::splat(action_button_dimension),
            );

            // the right to left layout is used to mimic LabelContent's buttons behavior and get a
            // better alignment
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(action_button_rect)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );

            button.ui(&mut child_ui);
        }
    }

    fn desired_width(&self, ui: &Ui) -> DesiredWidth {
        let layout_info = LayoutInfoStack::top(ui.ctx());

        if crate::is_in_resizable_panel(ui) {
            DesiredWidth::AtLeast(self.min_desired_width)
        } else if let Some(max_width) = layout_info.property_content_max_width {
            let mut desired_width = max_width + layout_info.left_x - ui.max_rect().left();

            // TODO(ab): ideally there wouldn't be as much code duplication with `Self::ui`
            let action_button_dimension =
                DesignTokens::small_icon_size().x + 2.0 * ui.spacing().button_padding.x;
            let reserve_action_button_space =
                self.button.is_some() || layout_info.reserve_action_button_space;
            if reserve_action_button_space {
                desired_width += action_button_dimension + DesignTokens::text_to_icon_padding();
            }

            DesiredWidth::AtLeast(desired_width.ceil())
        } else {
            DesiredWidth::AtLeast(self.min_desired_width)
        }
    }
}
