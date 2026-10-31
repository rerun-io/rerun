use egui::{Ui, Widget};

use crate::list_item::{
    ContentContext, DesiredWidth, ItemButtons, ListItemContent, ListItemContentButtonsExt,
};
use crate::{OnResponseExt as _, Size, UiExt as _};

/// Control how the [`CustomContent`] advertises its width.
#[derive(Debug, Clone, Copy)]
enum CustomContentDesiredWidth {
    /// Use the provided [`DesiredWidth`].
    DesiredWidth(DesiredWidth),

    /// Use [`DesiredWidth::AtLeast`] with a width computed from the provided content, plus any
    /// extras such as a button.
    ContentWidth(f32),
}

impl Default for CustomContentDesiredWidth {
    fn default() -> Self {
        Self::DesiredWidth(Default::default())
    }
}

/// [`ListItemContent`] that mostly delegates to a closure.
#[expect(clippy::type_complexity)]
pub struct CustomContent<'a> {
    ui: Box<dyn FnOnce(&mut egui::Ui, &ContentContext<'_>) + 'a>,
    desired_width: CustomContentDesiredWidth,

    buttons: ItemButtons<'a>,
}

impl<'a> CustomContent<'a> {
    /// Create a content with a custom UI closure.
    ///
    /// The closure will be called from within a [`egui::Ui`] with its maximum width set as per the
    /// list item geometry. Note that this may differ from [`ContentContext::rect`] if buttons are
    /// set.
    pub fn new(ui: impl FnOnce(&mut egui::Ui, &ContentContext<'_>) + 'a) -> Self {
        Self {
            ui: Box::new(ui),
            desired_width: Default::default(),
            buttons: ItemButtons::default().with_always_show_buttons(true),
        }
    }

    /// Set the desired width for the entire content.
    #[inline]
    pub fn with_desired_width(mut self, desired_width: DesiredWidth) -> Self {
        self.desired_width = CustomContentDesiredWidth::DesiredWidth(desired_width);
        self
    }

    /// Set the desired width based on the provided content width. If buttons are set, their width
    /// will be taken into account and added to the content width.
    #[inline]
    pub fn with_content_width(mut self, desired_content_width: f32) -> Self {
        self.desired_width = CustomContentDesiredWidth::ContentWidth(desired_content_width);
        self
    }

    /// Add a right-aligned button.
    ///
    /// Note: for aesthetics, space is always reserved for action buttons.
    #[inline]
    pub fn button(mut self, button: impl Widget + 'a) -> Self {
        self.buttons.add(button);
        self
    }

    /// Helper to add a button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn action_button(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        self.action_button_with_enabled(icon, alt_text, true, on_click)
    }

    /// Helper to add an enabled/disabled button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn action_button_with_enabled(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        enabled: bool,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        let alt_text = alt_text.into();
        self.button(move |ui: &mut Ui| {
            ui.add(
                ui.small_icon_button_widget(icon, &alt_text)
                    .on_click(on_click)
                    .enabled(enabled)
                    .on_hover_text(alt_text),
            )
        })
    }

    /// Helper to add a menu button to the right of the item.
    ///
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    ///
    /// See [`Self::button`] for more information.
    #[inline]
    pub fn menu_button(
        self,
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        add_contents: impl FnOnce(&mut egui::Ui) + 'a,
    ) -> Self {
        let alt_text = alt_text.into();
        self.button(|ui: &mut egui::Ui| {
            ui.add(
                ui.small_icon_button_widget(icon, &alt_text)
                    .on_menu(add_contents)
                    .on_hover_text(alt_text),
            )
        })
    }
}

impl ListItemContent for CustomContent<'_> {
    fn ui(
        self: Box<Self>,
        ui: &mut egui::Ui,
        context: &ContentContext<'_>,
    ) -> Option<egui::Response> {
        let Self {
            ui: content_ui,
            desired_width: _,
            buttons,
        } = *self;

        let mut content_rect = context.rect;
        buttons.show_and_shrink_rect(ui, context, &mut content_rect);

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                // When selected we override the text color so e.g. syntax highlighted code
                // doesn't become unreadable
                if context.visuals.selected {
                    ui.visuals_mut().override_text_color = Some(context.visuals.text_color());
                }
                content_ui(ui, context);
            },
        );

        None
    }

    fn desired_width(&self, ui: &Ui) -> DesiredWidth {
        match self.desired_width {
            CustomContentDesiredWidth::DesiredWidth(desired_width) => desired_width,
            CustomContentDesiredWidth::ContentWidth(mut content_width) => {
                if !self.buttons.is_empty() {
                    let tokens = ui.tokens();
                    let button_count = self.buttons.len() as f32;
                    // Reserve what the theme will give the buttons, not what the ambient button
                    // padding says.
                    let button_width = Size::inherited(ui).icon_button_size().x;
                    content_width += button_count * button_width
                        + (button_count - 1.0) * tokens.list_item_button_spacing()
                        + tokens.text_to_icon_padding();
                }
                DesiredWidth::AtLeast(content_width)
            }
        }
    }
}

impl<'a> ListItemContentButtonsExt<'a> for CustomContent<'a> {
    fn buttons(&self) -> &ItemButtons<'a> {
        &self.buttons
    }

    fn buttons_mut(&mut self) -> &mut ItemButtons<'a> {
        &mut self.buttons
    }
}
