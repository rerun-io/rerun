use crate::{Icon, ReUi};
use egui::{Response, Shape, Ui};

#[allow(clippy::type_complexity)]
pub struct ListItem<'a> {
    text: egui::WidgetText,
    re_ui: &'a ReUi,
    active: bool,
    selected: bool,
    icon: Option<&'a Icon>,
    buttons: Option<Box<dyn FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a>>,
}

impl<'a> ListItem<'a> {
    pub fn new(re_ui: &'a ReUi, text: impl Into<egui::WidgetText>) -> Self {
        Self {
            text: text.into(),
            re_ui,
            active: true,
            selected: false,
            icon: None,
            buttons: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn with_icon(mut self, icon: &'a Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_buttons(
        mut self,
        buttons: impl FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a,
    ) -> Self {
        self.buttons = Some(Box::new(buttons));
        self
    }
}

impl egui::Widget for ListItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let button_padding = ui.spacing().button_padding;
        let icon_width_plus_padding = ReUi::small_icon_size().x + ReUi::text_to_icon_padding();
        let icon_extra = if self.icon.is_some() {
            icon_width_plus_padding
        } else {
            0.0
        };
        let padding_extra = button_padding + button_padding;
        let wrap_width = ui.available_width() - padding_extra.x - icon_extra;

        let text =
            self.text
                .clone()
                .into_galley(ui, Some(false), wrap_width, egui::TextStyle::Button);

        let desired_size = egui::vec2(ui.available_width(), ReUi::list_item_height());
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

            let min_pos = ui.painter().round_pos_to_pixels(egui::pos2(
                rect.min.x.ceil(),
                ((rect.min.y + rect.max.y - ReUi::small_icon_size().y) * 0.5).ceil(),
            ));

            // Draw icon
            if let Some(icon) = self.icon {
                let icon_rect = egui::Rect::from_min_size(min_pos, ReUi::small_icon_size());

                let image = self.re_ui.icon_image(icon);
                let texture_id = image.texture_id(ui.ctx());
                let tint = visuals.fg_stroke.color;

                ui.painter().image(
                    texture_id,
                    icon_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
            }

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x = min_pos.x + icon_extra;
            let text_pos = ui
                .layout()
                .align_size_within_rect(text.size(), text_rect)
                .min;
            text.paint_with_visuals(ui.painter(), text_pos, &visuals);

            // Handle buttons
            let button_hovered =
                if self.active && ui.interact(rect, ui.id(), egui::Sense::hover()).hovered() {
                    if let Some(buttons) = self.buttons {
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
