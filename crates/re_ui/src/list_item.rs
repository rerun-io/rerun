use crate::{Icon, ReUi};
use egui::{Align2, NumExt, Response, Shape, Ui};

/// Generic widget for use in lists.
///
/// Layout:
/// ```text
/// ┌───────────────────────────────────────────────────┐
/// │┌──────┐                           ┌──────┐┌──────┐│
/// ││      │                           │      ││      ││
/// ││ icon │  label                    │ btns ││ btns ││
/// ││      │                           │      ││      ││
/// │└──────┘                           └──────┘└──────┘│
/// └───────────────────────────────────────────────────┘
/// ```
///
/// Features:
/// - selectable
/// - full span highlighting
/// - optional icon
/// - optional on-hover buttons on the right
///
/// This widget relies on the clip rectangle to be properly set as it use it for the shape if its
/// background highlighting. This has a significant impact on the hierarchy of the UI. This is
/// typically how things should be laid out:
///
/// ```text
/// Panel (no margin, set the clip rectangle)
/// └── ScrollArea (no margin)
///     └── Frame (with inner margin)
///         └── ListItem
/// ```
///
/// See [`ReUi::panel_content`] for an helper to build the [`egui::Frame`] with proper margins.
#[allow(clippy::type_complexity)]
pub struct ListItem<'a> {
    text: egui::WidgetText,
    re_ui: &'a ReUi,
    active: bool,
    selected: bool,
    icon_fn:
        Option<Box<dyn FnOnce(&ReUi, &mut egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a>>,
    buttons_fn: Option<Box<dyn FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a>>,
}

impl<'a> ListItem<'a> {
    /// Create a new [`ListItem`] with the given label.
    pub fn new(re_ui: &'a ReUi, text: impl Into<egui::WidgetText>) -> Self {
        Self {
            text: text.into(),
            re_ui,
            active: true,
            selected: false,
            icon_fn: None,
            buttons_fn: None,
        }
    }

    /// Set the active state the item.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set the selected state of the item.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the item.
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|re_ui, ui, rect, visuals| {
            let image = re_ui.icon_image(icon);
            let texture_id = image.texture_id(ui.ctx());
            let tint = visuals.fg_stroke.color;

            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    pub fn with_icon_fn(
        mut self,
        icon_fn: impl FnOnce(&ReUi, &mut egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a,
    ) -> Self {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Provide a closure to display on-hover buttons on the right of the item.
    ///
    /// Note that the a right to left layout is used, so the right-most button must be added first.
    pub fn with_buttons(
        mut self,
        buttons: impl FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a,
    ) -> Self {
        self.buttons_fn = Some(Box::new(buttons));
        self
    }

    /// Draw the item.
    pub fn show(self, ui: &mut Ui) -> Response {
        ui.add(self)
    }
}

impl egui::Widget for ListItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let button_padding = ui.spacing().button_padding;
        let icon_extra = if self.icon_fn.is_some() {
            ReUi::small_icon_size().x + ReUi::text_to_icon_padding()
        } else {
            0.0
        };
        let padding_extra = button_padding + button_padding;
        let wrap_width = ui.available_width() - padding_extra.x - icon_extra;

        let text =
            self.text
                .clone()
                .into_galley(ui, Some(false), wrap_width, egui::TextStyle::Button);

        let desired_size = (padding_extra + egui::vec2(icon_extra, 0.0) + text.size())
            .at_least(egui::vec2(ui.available_width(), ReUi::list_item_height()));
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click());

        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                self.selected,
                text.text(),
            )
        });

        if ui.is_rect_visible(rect) {
            let visuals = if self.active {
                ui.style().interact_selectable(&response, self.selected)
            } else {
                ui.visuals().widgets.inactive
            };

            let mut bg_rect = rect;
            bg_rect.extend_with_x(ui.clip_rect().right());
            bg_rect.extend_with_x(ui.clip_rect().left());
            let background_frame = ui.painter().add(egui::Shape::Noop);

            // Draw icon
            if let Some(icon_fn) = self.icon_fn {
                let icon_pos = ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x,
                    rect.center().y - 0.5 * ReUi::small_icon_size().y,
                ));
                let icon_rect = egui::Rect::from_min_size(icon_pos, ReUi::small_icon_size());
                icon_fn(self.re_ui, ui, icon_rect, visuals);
            }

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x += icon_extra;
            let text_pos = Align2::LEFT_CENTER
                .align_size_within_rect(text.size(), text_rect)
                .min;
            text.paint_with_visuals(ui.painter(), text_pos, &visuals);

            // Handle buttons
            let button_hovered =
                if self.active && ui.interact(rect, ui.id(), egui::Sense::hover()).hovered() {
                    if let Some(buttons) = self.buttons_fn {
                        let mut ui =
                            ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::Center));
                        buttons(self.re_ui, &mut ui).hovered()
                    } else {
                        false
                    }
                } else {
                    false
                };

            // Draw background on interaction.
            let bg_fill = if button_hovered {
                Some(visuals.bg_fill)
            } else if self.selected
                || response.hovered()
                || response.highlighted()
                || response.has_focus()
            {
                Some(visuals.weak_bg_fill)
            } else {
                None
            };

            if let Some(bg_fill) = bg_fill {
                ui.painter()
                    .set(background_frame, Shape::rect_filled(bg_rect, 0.0, bg_fill));
            }
        }

        response
    }
}
