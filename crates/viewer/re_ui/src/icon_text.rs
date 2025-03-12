use crate::{icon_text, icons, Icon};
use egui::{Context, ModifierNames, Modifiers, WidgetText};
use std::fmt::Debug;
use std::iter::once;
use std::{fmt, vec};

#[derive(Clone)]
pub enum IconTextItem {
    Icon(Icon),
    Text(WidgetText),
}

impl Debug for IconTextItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Icon(icon) => write!(f, "Icon({})", icon.id),
            Self::Text(text) => write!(f, "Text({})", text.text()),
        }
    }
}

impl IconTextItem {
    pub fn icon(icon: Icon) -> Self {
        Self::Icon(icon)
    }

    pub fn text(text: impl Into<WidgetText>) -> Self {
        Self::Text(text.into())
    }
}

/// Helper to show text with icons in a row.
/// Usually created via the [`crate::icon_text!`] macro.
#[derive(Default, Debug, Clone)]
pub struct IconText(pub Vec<IconTextItem>);

impl From<String> for IconText {
    fn from(value: String) -> Self {
        Self(vec![IconTextItem::Text(value.into())])
    }
}

impl From<&str> for IconText {
    fn from(value: &str) -> Self {
        Self(vec![IconTextItem::Text(value.into())])
    }
}

impl From<Icon> for IconText {
    fn from(icon: Icon) -> Self {
        Self(vec![IconTextItem::Icon(icon)])
    }
}

impl From<IconTextItem> for IconText {
    fn from(value: IconTextItem) -> Self {
        Self(vec![value])
    }
}

impl IconText {
    /// Create a new, empty `IconText`.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Add an icon to the row.
    pub fn icon(&mut self, icon: Icon) {
        self.0.push(IconTextItem::Icon(icon));
    }

    /// Add text to the row.
    pub fn text(&mut self, text: impl Into<WidgetText>) {
        self.0.push(IconTextItem::Text(text.into()));
    }

    /// Add an item to the row.
    pub fn add(&mut self, item: impl Into<Self>) {
        self.0.extend(item.into().0);
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

fn is_mac(ctx: &Context) -> bool {
    matches!(
        ctx.os(),
        egui::os::OperatingSystem::Mac | egui::os::OperatingSystem::IOS
    )
}

/// Shows a "+" if the OS is not Mac.
/// Useful if you want to e.g. show an icon after a [`modifiers_text`]
pub fn maybe_plus(ctx: &Context) -> IconText {
    if is_mac(ctx) {
        IconText::default()
    } else {
        icon_text!("+")
    }
}

/// Shows a keyboard shortcut with a modifier and the given icon.
///
/// On Mac, this will show the symbol for the modifier.
/// Otherwise, it will show the name of the modifier, and a "+" between the modifier and the icon.
pub fn shortcut_with_icon(
    ctx: &Context,
    modifiers: Modifiers,
    icon: impl Into<IconText>,
) -> IconText {
    icon_text!(modifiers_text(modifiers, ctx), maybe_plus(ctx), icon.into())
}

/// Helper to add [`egui::Modifiers`] as text with icons.
/// Will automatically show Cmd/Ctrl based on the OS.
pub fn modifiers_text(modifiers: Modifiers, ctx: &egui::Context) -> IconText {
    let is_mac = is_mac(ctx);

    let names = if is_mac {
        let mut names = ModifierNames::SYMBOLS;
        names.concat = "";
        names
    } else {
        let mut names = ModifierNames::NAMES;
        names.concat = "+";
        names
    };
    let text = names.format(&modifiers, is_mac);

    let mut icon_text = IconText::new();

    if is_mac {
        for char in text.chars() {
            if char == '⌘' {
                icon_text.add(IconTextItem::icon(icons::COMMAND));
            } else if char == '⌃' {
                icon_text.add(IconTextItem::icon(icons::CONTROL));
            } else if char == '⇧' {
                icon_text.add(IconTextItem::icon(icons::SHIFT));
            } else if char == '⌥' {
                icon_text.add(IconTextItem::icon(icons::OPTION));
            } else {
                // If there is anything else than the modifier symbols, just show the text.
                return text.into();
            }
        }
        icon_text
    } else {
        let mut vec: Vec<_> = text
            .split('+')
            .map(IconTextItem::text)
            // We want each + to be an extra item so the spacing looks nicer
            .flat_map(|item| once(item).chain(once(IconTextItem::text("+"))))
            .collect();
        vec.pop(); // Remove the last "+"
        IconText(vec)
    }
}

/// Helper to show mouse buttons as text/icons.
pub struct MouseButtonText(pub egui::PointerButton);

impl From<MouseButtonText> for IconText {
    fn from(value: MouseButtonText) -> Self {
        match value.0 {
            egui::PointerButton::Primary => icons::LEFT_MOUSE_CLICK.into(),
            egui::PointerButton::Secondary => icons::RIGHT_MOUSE_CLICK.into(),
            egui::PointerButton::Middle => "middle mouse button".into(),
            egui::PointerButton::Extra1 => "extra 1 mouse button".into(),
            egui::PointerButton::Extra2 => "extra 2 mouse button".into(),
        }
    }
}
