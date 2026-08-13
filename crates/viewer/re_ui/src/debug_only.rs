//! Helpers for marking the parts of the UI that only exist in debug builds.
//!
//! Anything the user can see that is gated behind `debug_assertions` should be marked
//! with one of these, so that it is obvious it won't be there in a release build.

use egui::{RichText, WidgetText};

use crate::{HasDesignTokens as _, egui_ext::concat_rich_text};

/// The text of the "debug only" marker.
///
/// The spaces are so the orange background reads as a badge.
pub const DEBUG_ONLY_TEXT: &str = " debug only ";

/// Air between the badge and whatever text precedes it, so the orange background doesn't
/// bump straight into it.
const SPACE_BEFORE_BADGE: &str = "  ";

/// Explains what the "debug only" marker means.
pub const DEBUG_ONLY_TOOLTIP: &str =
    "This is only shown in debug builds of the Rerun viewer, and is not part of release builds.";

/// The small orange "debug only" badge, as text that can be put anywhere.
pub fn debug_only_rich_text(style: &egui::Style) -> RichText {
    let tokens = style.tokens();
    RichText::new(DEBUG_ONLY_TEXT)
        .small()
        .color(tokens.alert_warning.icon)
        .background_color(tokens.alert_warning.fill)
}

/// `text`, followed by the orange "debug only" badge.
pub fn with_debug_only_badge(style: &egui::Style, text: impl Into<RichText>) -> WidgetText {
    concat_rich_text(
        style,
        [
            text.into(),
            SPACE_BEFORE_BADGE.into(),
            debug_only_rich_text(style),
        ],
    )
}

/// Append the orange "debug only" badge to the end of a [`egui::text::LayoutJob`].
pub fn append_debug_only_badge(job: &mut egui::text::LayoutJob, style: &egui::Style) {
    for text in [
        RichText::new(SPACE_BEFORE_BADGE),
        debug_only_rich_text(style),
    ] {
        text.append_to(
            job,
            style,
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    }
}

/// Show the orange "debug only" badge on its own.
pub fn debug_only_badge_ui(ui: &mut egui::Ui) -> egui::Response {
    let text = debug_only_rich_text(ui.style());
    ui.add(egui::Label::new(text).selectable(false))
        .on_hover_text(DEBUG_ONLY_TOOLTIP)
}
