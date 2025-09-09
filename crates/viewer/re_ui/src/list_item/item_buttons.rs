use crate::UiExt;
use crate::list_item::ContentContext;
use egui::Widget;

#[derive(Default)]
pub struct ItemButtons<'a>(Vec<Box<dyn FnOnce(&mut egui::Ui) + 'a>>);

impl Clone for ItemButtons<'_> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for ItemButtons<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Buttons").field(&self.0.len()).finish()
    }
}

impl<'a> ItemButtons<'a> {
    pub fn add(&mut self, button: impl Widget + 'a) {
        self.0.push(Box::new(move |ui: &mut egui::Ui| {
            button.ui(ui);
        }));
    }

    pub fn add_buttons(&mut self, buttons: impl FnOnce(&mut egui::Ui) + 'a) {
        self.0.push(Box::new(buttons));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn should_show_buttons(context: &ContentContext) -> bool {
        // We can't use `.hovered()` or the buttons disappear just as the user clicks,
        // so we use `contains_pointer` instead. That also means we need to check
        // that we aren't dragging anything.
        // By showing the buttons when selected, we allow users to find them on touch screens.
        (context.list_item.interactive
            && context
                .response
                .ctx
                .rect_contains_pointer(context.response.layer_id, context.bg_rect)
            && !egui::DragAndDrop::has_any_payload(&context.response.ctx))
            || context.list_item.selected
    }

    pub fn show_and_shrink_rect(
        self,
        ui: &mut egui::Ui,
        context: &ContentContext,
        always_show: bool,
        rect: &mut egui::Rect,
    ) {
        if self.0.is_empty() || !(Self::should_show_buttons(context) || always_show) {
            return;
        }

        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(*rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );

        let tokens = ui.tokens();
        if context.list_item.selected {
            // Icons and text get different colors when they are on a selected background:
            let visuals = ui.visuals_mut();

            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.weak_bg_fill = tokens.surface_on_primary_hovered;
            visuals.widgets.hovered.weak_bg_fill = tokens.surface_on_primary_hovered;

            visuals.widgets.noninteractive.fg_stroke.color = tokens.icon_color_on_primary;
            visuals.widgets.inactive.fg_stroke.color = tokens.icon_color_on_primary;
            visuals.widgets.active.fg_stroke.color = tokens.icon_color_on_primary_hovered;
            visuals.widgets.hovered.fg_stroke.color = tokens.icon_color_on_primary_hovered;
        }

        for button in self.0 {
            button(&mut ui);
        }

        let used_rect = ui.min_rect();
        rect.max.x -= used_rect.width() + tokens.text_to_icon_padding();
    }
}

pub trait ListItemContentButtonsExt<'a>
where
    Self: Sized,
{
    fn buttons(&self) -> &ItemButtons<'a>;
    fn buttons_mut(&mut self) -> &mut ItemButtons<'a>;

    fn button(mut self, button: impl Widget + 'a) -> Self {
        self.buttons_mut().add(button);
        self
    }

    fn buttons_fn(mut self, buttons: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        self.buttons_mut().add_buttons(buttons);
        self
    }
}
