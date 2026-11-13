//! The classes that no single type owns, and the lookups that read a class vocabulary.
//!
//! A class that belongs to one type lives on that type instead, as a `CLASS_*` associated const —
//! see [`crate::Variant`], [`crate::Size`] and [`crate::text_edit::TextEditVariant`], the way
//! [`egui::Button::CLASS_SELECTED`] does it.

use egui::UiStack;
use egui::class::Classes;
use egui::widget_style::StyleArgs;

/// Make the widget icon-shaped: square, sized by [`crate::Size`], with no padding.
pub const ICON: &str = "rerun::icon";

/// Show the widget as if the cursor were hovering over it.
pub const HIGHLIGHTED: &str = "rerun::highlighted";

/// Let the Rerun variant decide how a selected widget looks.
///
/// Normally [`egui::Button::CLASS_SELECTED`] takes precedence over the variant classes; with this
/// class present the variant wins.
pub const SELECTED_VARIANT: &str = "rerun::selected_variant";

/// A field whose current content is invalid.
///
/// The theme outlines it in the error color.
pub const INVALID: &str = "rerun::invalid";

// --- Set on a container, and inherited by everything inside it ---

/// Tell the widgets inside to fit the filled background of a selected list item.
pub const ON_SELECTED: &str = "rerun::on_selected";

/// Tell the widgets inside to fit the background of a hovered or active list item.
///
/// That background is the same shade as a button's hover fill, so the buttons step one shade past it.
pub const ON_HOVERED: &str = "rerun::on_hovered";

/// Make the buttons inside gray rather than blue while selected.
///
/// Used by the `selectable_toggle`.
pub const SELECTED_NEUTRAL: &str = "rerun::selected_neutral";

/// Mark UI built by a crate we don't control, such as `egui_commonmark`.
///
/// That code styles its widgets with egui's own setters (e.g. `Button::small`), which the theme
/// otherwise rejects in favor of a [`crate::Variant`] and a [`crate::Size`]. Inside this class the
/// theme maps them as best it can instead.
pub const UNCONTROLLED_EXTERNAL_UI: &str = "rerun::uncontrolled_external_ui";

// --- Lookups ---
//
// egui's own themes only ever ask whether a class is present, so these live here rather than in
// `egui::widget_style`.

/// Read a value out of a list of classes.
pub trait ClassesExt {
    /// The last class that names a `T`, if any.
    ///
    /// Scanning backwards is what makes the last class win, so a widget given two classes of the
    /// same kind takes the one set last — the way a later stylesheet rule overrides an earlier one.
    fn last_class<T>(&self, from_class_name: impl Fn(&str) -> Option<T>) -> Option<T>;
}

impl ClassesExt for Classes {
    fn last_class<T>(&self, from_class_name: impl Fn(&str) -> Option<T>) -> Option<T> {
        self.as_slice()
            .iter()
            .rev()
            .find_map(|class| from_class_name(class))
    }
}

/// Read a value out of the classes of a [`egui::Ui`] and its parents.
pub trait UiStackExt {
    /// Read a value from the classes up the stack, nearest [`egui::Ui`] first.
    ///
    /// This is how an inherited property works in a stylesheet: a container states it once, and
    /// everything inside picks it up. The nearest [`egui::Ui`] wins, so an inner container can
    /// override an outer one.
    fn inherited<T>(&self, from_classes: impl Fn(&Classes) -> Option<T>) -> Option<T>;
}

impl UiStackExt for UiStack {
    fn inherited<T>(&self, from_classes: impl Fn(&Classes) -> Option<T>) -> Option<T> {
        self.iter().find_map(|node| from_classes(&node.classes))
    }
}

/// Read a value out of the classes a [`egui::theme::StyleProvider`] was called with.
pub trait StyleArgsExt {
    /// Read a value from the widget's own classes, falling back to the [`egui::Ui`]s it sits in.
    ///
    /// See [`UiStackExt::inherited`] for how the fallback resolves.
    fn inherited<T>(&self, from_classes: impl Fn(&Classes) -> Option<T>) -> Option<T>;
}

impl StyleArgsExt for StyleArgs<'_> {
    fn inherited<T>(&self, from_classes: impl Fn(&Classes) -> Option<T>) -> Option<T> {
        from_classes(self.classes).or_else(|| self.stack.inherited(from_classes))
    }
}
