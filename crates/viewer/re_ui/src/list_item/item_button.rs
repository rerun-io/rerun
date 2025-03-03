//! Abstraction for buttons to be used in list items.

use crate::{Icon, UiExt as _};
use egui::containers::menu::MenuButton;

// -------------------------------------------------------------------------------------------------

/// An [`super::ItemButton`] that acts as a popup menu.
pub struct ItemMenuButton<'a> {
    icon: &'static Icon,
    add_contents: Box<dyn FnOnce(&mut egui::Ui) + 'a>,
    enabled: bool,
    hover_text: Option<String>,
    disabled_hover_text: Option<String>,
}

impl<'a> ItemMenuButton<'a> {
    pub fn new(icon: &'static Icon, add_contents: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        Self {
            icon,
            add_contents: Box::new(add_contents),
            enabled: true,
            hover_text: None,
            disabled_hover_text: None,
        }
    }

    /// Sets enable/disable state of the button.
    #[inline]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets text shown when the button hovered.
    #[inline]
    pub fn hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.hover_text = Some(hover_text.into());
        self
    }

    /// Sets text shown when the button is disabled and hovered.
    #[inline]
    pub fn disabled_hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.disabled_hover_text = Some(hover_text.into());
        self
    }
}

impl super::ItemButton for ItemMenuButton<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui) -> egui::Response {
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let (mut response, _) = MenuButton::from_button(ui.small_icon_button_widget(self.icon))
                .ui(ui, self.add_contents);
            if let Some(hover_text) = self.hover_text {
                response = response.on_hover_text(hover_text);
            }
            if let Some(disabled_hover_text) = self.disabled_hover_text {
                response = response.on_disabled_hover_text(disabled_hover_text);
            }

            response
        })
        .inner
    }
}

// -------------------------------------------------------------------------------------------------

/// An [`super::ItemButton`] that acts as an action button.
pub struct ItemActionButton<'a> {
    icon: &'static crate::icons::Icon,
    on_click: Box<dyn FnOnce() + 'a>,
    enabled: bool,
    hover_text: Option<String>,
    disabled_hover_text: Option<String>,
}

impl<'a> ItemActionButton<'a> {
    pub fn new(icon: &'static crate::icons::Icon, on_click: impl FnOnce() + 'a) -> Self {
        Self {
            icon,
            on_click: Box::new(on_click),
            enabled: true,
            hover_text: None,
            disabled_hover_text: None,
        }
    }

    /// Sets enable/disable state of the button.
    #[inline]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets text shown when the button hovered.
    #[inline]
    pub fn hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.hover_text = Some(hover_text.into());
        self
    }

    /// Sets text shown when the button is disabled and hovered.
    #[inline]
    pub fn disabled_hover_text(mut self, hover_text: impl Into<String>) -> Self {
        self.disabled_hover_text = Some(hover_text.into());
        self
    }
}

impl super::ItemButton for ItemActionButton<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui) -> egui::Response {
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let mut response = ui.small_icon_button(self.icon);
            if response.clicked() {
                (self.on_click)();
            }

            if let Some(hover_text) = self.hover_text {
                response = response.on_hover_text(hover_text);
            }
            if let Some(disabled_hover_text) = self.disabled_hover_text {
                response = response.on_disabled_hover_text(disabled_hover_text);
            }

            response
        })
        .inner
    }
}
