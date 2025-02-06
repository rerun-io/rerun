use crate::{icons, Icon};
use egui::{ModifierNames, Modifiers};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum IconTextItem<'a> {
    Icon(Icon),
    Text(Cow<'a, str>),
}

impl<'a> IconTextItem<'a> {
    pub fn icon(icon: Icon) -> Self {
        Self::Icon(icon)
    }

    pub fn text(text: impl Into<Cow<'a, str>>) -> Self {
        Self::Text(text.into())
    }
}

/// Helper to show text with icons in a row.
/// Usually created via the [`crate::icon_text!`] macro.
#[derive(Default, Debug, Clone)]
pub struct IconText<'a>(pub Vec<IconTextItem<'a>>);

impl<'a> IconText<'a> {
    /// Create a new, empty `IconText`.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Add an icon to the row.
    pub fn icon(&mut self, icon: Icon) {
        self.0.push(IconTextItem::Icon(icon));
    }

    /// Add text to the row.
    pub fn text(&mut self, text: impl Into<Cow<'a, str>>) {
        self.0.push(IconTextItem::Text(text.into()));
    }

    /// Add an item to the row.
    pub fn add(&mut self, item: impl Into<IconTextItem<'a>>) {
        self.0.push(item.into());
    }
}

impl<'a> From<Icon> for IconTextItem<'a> {
    fn from(icon: Icon) -> Self {
        IconTextItem::Icon(icon)
    }
}

impl<'a> From<&'a str> for IconTextItem<'a> {
    fn from(text: &'a str) -> Self {
        IconTextItem::Text(text.into())
    }
}

impl<'a> From<String> for IconTextItem<'a> {
    fn from(text: String) -> Self {
        IconTextItem::Text(text.into())
    }
}

/// Create an [`IconText`] with the given items.
#[macro_export]
macro_rules! icon_text {
    ($($item:expr),* $(,)?) => {{
        let mut icon_text = $crate::IconText::new();
        $(icon_text.add($item);)*
        icon_text
    }};
}

/// Helper to add [`egui::Modifiers`] as text with icons.
/// Will automatically show Cmd/Ctrl based on the OS.
pub struct ModifiersText<'a>(pub Modifiers, pub &'a egui::Context);

impl<'a> From<ModifiersText<'a>> for IconTextItem<'static> {
    fn from(value: ModifiersText<'a>) -> Self {
        let ModifiersText(modifiers, ctx) = value;

        let is_mac = matches!(
            ctx.os(),
            egui::os::OperatingSystem::Mac | egui::os::OperatingSystem::IOS
        );

        let mut names = ModifierNames::NAMES;
        names.concat = " + ";
        let text = names.format(&modifiers, is_mac);

        // Only shift has an icon for now
        if text == "Shift" {
            IconTextItem::Icon(icons::SHIFT)
        } else {
            IconTextItem::text(text)
        }
    }
}

/// Helper to show mouse buttons as text/icons.
pub struct MouseButtonText(pub egui::PointerButton);

impl From<MouseButtonText> for IconTextItem<'static> {
    fn from(value: MouseButtonText) -> Self {
        match value.0 {
            egui::PointerButton::Primary => IconTextItem::icon(icons::LEFT_MOUSE_CLICK),
            egui::PointerButton::Secondary => IconTextItem::icon(icons::RIGHT_MOUSE_CLICK),
            egui::PointerButton::Middle => IconTextItem::text("middle mouse button"),
            egui::PointerButton::Extra1 => IconTextItem::text("extra 1 mouse button"),
            egui::PointerButton::Extra2 => IconTextItem::text("extra 2 mouse button"),
        }
    }
}
