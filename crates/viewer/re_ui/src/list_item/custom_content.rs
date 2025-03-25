use egui::{NumExt as _, Ui};

use crate::list_item::{ContentContext, DesiredWidth, ListItemContent};
use crate::DesignTokens;

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

    //TODO(ab): in the future, that should be a `Vec`, with some auto expanding mini-toolbar
    button: Option<Box<dyn super::ItemButton + 'a>>,
}

impl<'a> CustomContent<'a> {
    /// Create a content with a custom UI closure.
    ///
    /// The closure will be called from within a [`egui::Ui`] with its maximum width set as per the
    /// list item geometry. Note that this may differ from [`ContentContext::rect`] if a button is
    /// set.
    pub fn new(ui: impl FnOnce(&mut egui::Ui, &ContentContext<'_>) + 'a) -> Self {
        Self {
            ui: Box::new(ui),
            desired_width: Default::default(),
            button: None,
        }
    }

    /// Set the desired width for the entire content.
    #[inline]
    pub fn with_desired_width(mut self, desired_width: DesiredWidth) -> Self {
        self.desired_width = CustomContentDesiredWidth::DesiredWidth(desired_width);
        self
    }

    /// Set the desired width based on the provided content width. If a button is set, its width
    /// will be taken into account and added to the content width.
    #[inline]
    pub fn with_content_width(mut self, desired_content_width: f32) -> Self {
        self.desired_width = CustomContentDesiredWidth::ContentWidth(desired_content_width);
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
}

impl ListItemContent for CustomContent<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        let Self {
            ui: content_ui,
            desired_width: _,
            button,
        } = *self;

        let button_dimension =
            DesignTokens::small_icon_size().x + 2.0 * ui.spacing().button_padding.x;

        let content_width = if button.is_some() {
            (context.rect.width() - button_dimension - DesignTokens::text_to_icon_padding())
                .at_least(0.0)
        } else {
            context.rect.width()
        };

        let content_rect = egui::Rect::from_min_size(
            context.rect.min,
            egui::vec2(content_width, context.rect.height()),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                content_ui(ui, context);
            },
        );

        if let Some(button) = button {
            let action_button_rect = egui::Rect::from_center_size(
                context.rect.right_center() - egui::vec2(button_dimension / 2.0, 0.0),
                egui::Vec2::splat(button_dimension),
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
        match self.desired_width {
            CustomContentDesiredWidth::DesiredWidth(desired_width) => desired_width,
            CustomContentDesiredWidth::ContentWidth(mut content_width) => {
                if self.button.is_some() {
                    content_width += DesignTokens::small_icon_size().x
                        + 2.0 * ui.spacing().button_padding.x
                        + DesignTokens::text_to_icon_padding();
                }
                DesiredWidth::AtLeast(content_width)
            }
        }
    }
}
