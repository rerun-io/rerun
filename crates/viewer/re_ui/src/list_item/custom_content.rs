use egui::{NumExt as _, Ui};

use crate::UiExt as _;
use crate::list_item::{
    ContentContext, DesiredWidth, ItemButtons, ListItemContent, ListItemContentButtonsExt,
};

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
    /// list item geometry. Note that this may differ from [`ContentContext::rect`] if a button is
    /// set.
    pub fn new(ui: impl FnOnce(&mut egui::Ui, &ContentContext<'_>) + 'a) -> Self {
        Self {
            ui: Box::new(ui),
            desired_width: Default::default(),
            buttons: ItemButtons::default().with_extend_on_overflow(true),
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
}

impl ListItemContent for CustomContent<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui, context: &ContentContext<'_>) {
        let Self {
            ui: content_ui,
            desired_width: _,
            buttons,
        } = *self;

        let mut content_rect = context.rect;
        let buttons_rect = buttons.show(ui, context, content_rect, |ui| {
            // When selected we override the text color so e.g. syntax highlighted code
            // doesn't become unreadable
            if context.visuals.selected {
                ui.visuals_mut().override_text_color = Some(context.visuals.text_color());
            }
            content_ui(ui, context);
        });

        // context.layout_info.register_max_item_width(
        //     ui.ctx(),
        //     response.response.rect.width()
        //         + ui.tokens().text_to_icon_padding()
        //         + buttons_rect.width(),
        // )
    }

    fn desired_width(&self, ui: &Ui) -> DesiredWidth {
        match self.desired_width {
            CustomContentDesiredWidth::DesiredWidth(desired_width) => desired_width,
            CustomContentDesiredWidth::ContentWidth(mut content_width) => {
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
