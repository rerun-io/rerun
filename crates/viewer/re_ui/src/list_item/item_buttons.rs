use egui::Widget;

use crate::list_item::ContentContext;
use crate::{OnResponseExt as _, UiExt as _};

type ButtonFn<'a> = Box<dyn FnOnce(&mut egui::Ui) + 'a>;

#[derive(Default)]
pub struct ItemButtons<'a> {
    buttons: Vec<ButtonFn<'a>>,
    always_show_buttons: bool,
}

impl Clone for ItemButtons<'_> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for ItemButtons<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Buttons").field(&self.buttons.len()).finish()
    }
}

impl<'a> ItemButtons<'a> {
    pub fn add(&mut self, button: impl Widget + 'a) {
        self.buttons.push(Box::new(move |ui: &mut egui::Ui| {
            button.ui(ui);
        }));
    }

    pub fn add_buttons(&mut self, buttons: impl FnOnce(&mut egui::Ui) + 'a) {
        self.buttons.push(Box::new(buttons));
    }

    pub fn is_empty(&self) -> bool {
        self.buttons.is_empty()
    }

    fn should_show_buttons(&self, context: &ContentContext<'_>) -> bool {
        // We can't use `.hovered()` or the buttons disappear just as the user clicks,
        // so we use `contains_pointer` instead. That also means we need to check
        // that we aren't dragging anything.
        // By showing the buttons when selected, we allow users to find them on touch screens.
        (context
            .response
            .ctx
            .rect_contains_pointer(context.response.layer_id, context.bg_rect)
            && !egui::DragAndDrop::has_any_payload(&context.response.ctx))
            || context.list_item.selected
            || self.always_show_buttons
    }

    pub fn show_and_shrink_rect(
        self,
        ui: &mut egui::Ui,
        context: &ContentContext<'_>,
        rect: &mut egui::Rect,
    ) {
        if self.buttons.is_empty() || !self.should_show_buttons(context) {
            ui.skip_ahead_auto_ids(1); // Make sure the id of `ui` remains the same after the call regardless
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

        for button in self.buttons {
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

    /// Add a single widget.
    ///
    /// It will be shown on the right side of the list item.
    /// By default, buttons are only shown on hover or when selected, use
    /// [`Self::with_always_show_buttons`] to change that.
    ///
    /// Usually you want to add an [`crate::UiExt::small_icon_button`] and use the helpers from
    /// [`crate::OnResponseExt`] to add actions / menus.
    ///
    /// Notes:
    /// - If buttons are used, the item will allocate the full available width of the parent. If the
    ///   enclosing UI adapts to the childrens width, it will unnecessarily grow. If buttons aren't
    ///   used, the item will only allocate the width needed for the text and icons if any.
    /// - A right to left layout is used, so the right-most button must be added first.
    #[inline]
    fn with_button(mut self, button: impl Widget + 'a) -> Self {
        self.buttons_mut().add(button);
        self
    }

    /// Add some content in the button area.
    ///
    /// It will be shown on the right side of the list item.
    /// By default, buttons are only shown on hover or when selected, use
    /// [`Self::with_always_show_buttons`] to change that.
    ///
    /// Usually you want to add an [`crate::UiExt::small_icon_button`] and use the helpers from
    /// [`crate::OnResponseExt`] to add actions / menus.
    ///
    /// Notes:
    /// - If buttons are used, the item will allocate the full available width of the parent. If the
    ///   enclosing UI adapts to the childrens width, it will unnecessarily grow. If buttons aren't
    ///   used, the item will only allocate the width needed for the text and icons if any.
    /// - A right to left layout is used, so the right-most button must be added first.
    #[inline]
    fn with_buttons(mut self, buttons: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        self.buttons_mut().add_buttons(buttons);
        self
    }

    /// Always show the buttons.
    ///
    /// By default, buttons are only shown when the item is hovered or selected. By setting this to
    /// `true`, the buttons are always shown.
    #[inline]
    fn with_always_show_buttons(mut self, always_show: bool) -> Self {
        self.buttons_mut().always_show_buttons = always_show;
        self
    }

    /// Helper to add a button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    /// The `alt_text` will also be used for the tooltip.
    ///
    /// See [`Self::with_button`] for more information.
    #[inline]
    fn with_action_button(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        self.with_action_button_enabled(icon, alt_text, true, on_click)
    }

    /// Helper to add an enabled/disabled button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    /// The `alt_text` will also be used for the tooltip.
    ///
    /// See [`Self::with_button`] for more information.
    #[inline]
    fn with_action_button_enabled(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        enabled: bool,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        let hover_text = alt_text.into();
        self.with_button(move |ui: &mut egui::Ui| {
            let thing = ui
                .small_icon_button_widget(icon, &hover_text)
                .on_click(on_click)
                .enabled(enabled)
                .on_hover_text(hover_text);
            ui.add(thing)
        })
    }

    /// Helper to add a menu button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    /// The `alt_text` will also be used for the tooltip.
    ///
    /// See [`Self::with_button`] for more information.
    ///
    /// Sets [`Self::with_always_show_buttons`] to `true` (TODO(emilk/egui#7531)).
    #[inline]
    fn with_menu_button(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        add_contents: impl FnOnce(&mut egui::Ui) + 'a,
    ) -> Self {
        let hover_text = alt_text.into();
        self.with_always_show_buttons(true)
            .with_button(|ui: &mut egui::Ui| {
                ui.add(
                    ui.small_icon_button_widget(icon, &hover_text)
                        .on_hover_text(hover_text)
                        .on_menu(add_contents),
                )
            })
    }

    /// Set the help text tooltip to be shown in the header.
    ///
    /// Sets [`Self::with_always_show_buttons`] to `true` (TODO(emilk/egui#7531)).
    #[inline]
    fn with_help_text(self, help: impl Into<egui::WidgetText> + 'a) -> Self {
        self.with_help_ui(|ui| {
            ui.label(help);
        })
    }

    /// Set the help markdown tooltip to be shown in the header.
    ///
    /// Sets [`Self::with_always_show_buttons`] to `true` (TODO(emilk/egui#7531)).
    #[inline]
    fn with_help_markdown(self, help: &'a str) -> Self {
        self.with_help_ui(|ui| {
            ui.markdown_ui(help);
        })
    }

    /// Set the help UI closure to be shown in the header.
    ///
    /// Sets [`Self::with_always_show_buttons`] to `true` (TODO(emilk/egui#7531)).
    #[inline]
    fn with_help_ui(self, help: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        self.with_always_show_buttons(true)
            .with_button(|ui: &mut egui::Ui| ui.help_button(help))
    }
}

impl<'a> ListItemContentButtonsExt<'a> for ItemButtons<'a> {
    fn buttons(&self) -> &Self {
        self
    }

    fn buttons_mut(&mut self) -> &mut Self {
        self
    }
}
