//! Abstraction for buttons to be used in list items.

use crate::menu::menu_style;
use crate::{Icon, UiExt as _};
use egui::containers::menu::{MenuButton, MenuConfig};
// -------------------------------------------------------------------------------------------------

/// An [`super::ItemButton`] that acts as a popup menu.
pub struct ItemMenuButton<'a> {
    icon: &'static Icon,
    alt_text: String,
    add_contents: Box<dyn FnOnce(&mut egui::Ui) + 'a>,
    enabled: bool,
    hover_text: Option<String>,
    disabled_hover_text: Option<String>,
    config: Option<MenuConfig>,
}

impl<'a> ItemMenuButton<'a> {
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    pub fn new(
        icon: &'static Icon,
        alt_text: impl Into<String>,
        add_contents: impl FnOnce(&mut egui::Ui) + 'a,
    ) -> Self {
        Self {
            icon,
            alt_text: alt_text.into(),
            add_contents: Box::new(add_contents),
            enabled: true,
            hover_text: None,
            disabled_hover_text: None,
            config: Default::default(),
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

    #[inline]
    pub fn config(mut self, config: MenuConfig) -> Self {
        self.config = Some(config);
        self
    }
}

impl super::ItemButton for ItemMenuButton<'_> {
    fn ui(self: Box<Self>, ui: &mut egui::Ui) -> egui::Response {
        let Self {
            icon,
            alt_text,
            add_contents,
            enabled,
            hover_text,
            disabled_hover_text,
            config,
        } = *self;

        ui.add_enabled_ui(enabled, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let mut button = MenuButton::from_button(ui.small_icon_button_widget(icon, alt_text))
                .config(config.unwrap_or_default().style(menu_style()));

            let (mut response, _) = button.ui(ui, add_contents);
            if let Some(hover_text) = hover_text {
                response = response.on_hover_text(hover_text);
            }
            if let Some(disabled_hover_text) = disabled_hover_text {
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
    alt_text: String,
    on_click: Box<dyn FnOnce() + 'a>,
    enabled: bool,
    hover_text: Option<String>,
    disabled_hover_text: Option<String>,
}

impl<'a> ItemActionButton<'a> {
    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    pub fn new(
        icon: &'static crate::icons::Icon,
        alt_text: impl Into<String>,
        on_click: impl FnOnce() + 'a,
    ) -> Self {
        Self {
            icon,
            alt_text: alt_text.into(),
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
        let Self {
            icon,
            alt_text,
            on_click,
            enabled,
            hover_text,
            disabled_hover_text,
        } = *self;

        ui.add_enabled_ui(enabled, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let mut response = ui.small_icon_button(icon, alt_text);
            if response.clicked() {
                (on_click)();
            }

            if let Some(hover_text) = hover_text {
                response = response.on_hover_text(hover_text);
            }
            if let Some(disabled_hover_text) = disabled_hover_text {
                response = response.on_disabled_hover_text(disabled_hover_text);
            }

            response
        })
        .inner
    }
}
