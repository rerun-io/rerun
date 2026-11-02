//! How [`egui::Button`] (and anything built from one, like [`egui::DragValue`]) is styled.

use egui::class::HasClasses as _;
use egui::theme::StyleProvider;
use egui::widget_style::{AtomLayoutStyle, ButtonStyle, StyleArgs, TextVisuals, WidgetState};
use egui::{Button, Color32, CornerRadius, Frame, Margin, Stroke, TextStyle, UiKind, Vec2};

use super::class::StyleArgsExt as _;
use super::{ReTheme, class};
use crate::button::{Size, Variant};
use crate::{ButtonVisuals, DesignTokens, HasDesignTokens as _};

impl StyleProvider<ButtonStyle> for ReTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> ButtonStyle {
        debug_assert_supported(args);

        let Colors { fill, text, stroke } = colors(args);
        let Metrics {
            padding,
            corner_radius,
            min_size,
            gap,
            outer_margin,
        } = metrics(args);

        let text_style = TextVisuals::new(args.style, TextStyle::Body, text);

        let painted_frame = Frame {
            fill,
            stroke,
            corner_radius,
            // Compensate for the stroke width, so there is no layout shift when stroke width
            // changes.
            inner_margin: (padding - Vec2::splat(stroke.width)).into(),
            outer_margin,
            ..Default::default()
        };

        ButtonStyle {
            atom_layout: AtomLayoutStyle {
                min_size,
                gap,
                // A button with no frame takes up no more room than its contents.
                frame: if args.style.visuals.button_frame {
                    painted_frame
                } else {
                    Frame::new()
                },
                // An untinted image follows the text color, if the button asked for that.
                image_tint: if args
                    .classes
                    .has_class(Button::CLASS_IMAGE_TINT_FOLLOWS_TEXT_COLOR.as_str())
                {
                    text_style.color
                } else {
                    Color32::WHITE
                },
                text_style,
                ..Default::default()
            },
        }
    }
}

/// The built-in [`Button`] classes this theme does not implement.
///
/// [`Button`] doesn't frame or size itself — those setters only attach a class, and it is up to
/// the theme to read it. Nothing in the viewer sets these, and a button's look should come from a
/// [`Variant`] and a [`Size`] rather than from egui's own knobs, so catch a stray call here instead
/// of silently ignoring it.
///
/// UI from a crate we don't control can't follow that rule, so it is exempt inside
/// [`class::UNCONTROLLED_EXTERNAL_UI`].
///
/// Only classes the caller sets deliberately can be asserted on. [`Button::CLASS_FRAME`] and
/// [`Button::CLASS_HIDE_FRAME_WHEN_INACTIVE`] cannot: egui attaches them on the caller's behalf.
/// `fill` and `stroke` imply `frame(true)`, and both are honored — egui applies them over the frame
/// this theme returns — while every `selectable_label` is built with `frame(true)` and, when it is
/// not selected, `frame_when_inactive(false)`.
fn debug_assert_supported(args: &StyleArgs<'_>) {
    if args.has_class(class::UNCONTROLLED_EXTERNAL_UI) {
        return;
    }

    for (class, setter) in [
        (Button::CLASS_NO_FRAME, "frame(false)"),
        (Button::CLASS_SMALL, "small()"),
    ] {
        re_log::debug_assert!(
            !args.classes.has_class(class.as_str()),
            "Rerun's theme ignores Button::{setter}. Use a `Variant` and a `Size` instead."
        );
    }
}

/// The buttons colors after applying all classes.
struct Colors {
    fill: Color32,
    text: Color32,
    stroke: Stroke,
}

fn colors(args: &StyleArgs<'_>) -> Colors {
    let tokens = args.style.tokens();
    let state = state(args);

    // A suggestion is selected while it is navigated to with the keyboard, which should look like
    // hovering it rather than like a chosen value.
    if args.classes.has_class(Button::CLASS_SELECTED.as_str())
        && args
            .classes
            .has_class(egui::CompletionPopup::CLASS_SUGGESTION.as_str())
    {
        return Colors {
            fill: tokens.list_item_hovered_bg,
            text: tokens.text_default,
            stroke: Stroke::NONE,
        };
    }

    if args.has_class(class::ON_SELECTED) {
        // On the filled background of a selected row the variant's own fill would vanish and its
        // text would be unreadable, so the colors come from the on-primary tokens instead.
        return if matches!(state, WidgetState::Hovered | WidgetState::Active) {
            Colors {
                fill: tokens.surface_on_primary_hovered,
                text: tokens.icon_color_on_primary_hovered,
                stroke: Stroke::NONE,
            }
        } else {
            Colors {
                fill: Color32::TRANSPARENT,
                text: tokens.icon_color_on_primary,
                stroke: Stroke::NONE,
            }
        };
    }

    let visuals = visuals(args, tokens);
    let (fill_hovered, fill_pressed) = if args.has_class(class::ON_HOVERED) {
        (
            tokens.list_item_button_hovered_bg,
            tokens.list_item_button_pressed_bg,
        )
    } else {
        (visuals.fill_hovered, visuals.fill_pressed)
    };

    Colors {
        fill: match state {
            WidgetState::Hovered => fill_hovered,
            WidgetState::Active => fill_pressed,
            WidgetState::Inactive | WidgetState::Noninteractive => visuals.fill,
        },
        text: visuals.text,
        stroke: visuals.stroke,
    }
}

/// Apply any overrides to the widget state.
fn state(args: &StyleArgs<'_>) -> WidgetState {
    // An open combo box is engaged the same way a highlighted button is.
    let engaged = args.classes.has_class(class::HIGHLIGHTED)
        || args.classes.has_class(egui::ComboBox::CLASS_OPEN.as_str());
    if args.state == WidgetState::Inactive && engaged {
        WidgetState::Hovered
    } else {
        args.state
    }
}

/// The colors of the variant the button ends up with, whether it is selected.
fn visuals(args: &StyleArgs<'_>, tokens: &DesignTokens) -> ButtonVisuals {
    let classes = args.classes;
    let visuals = Variant::from_classes(classes)
        .unwrap_or_default()
        .visuals(tokens);

    if !classes.has_class(Button::CLASS_SELECTED.as_str())
        || classes.has_class(class::SELECTED_VARIANT)
    {
        return visuals;
    }

    // Selected buttons take the selection accent, unless a container they sit in asked for a
    // neutral selected look.
    let selected = if args.has_class(class::SELECTED_NEUTRAL) {
        Variant::Secondary.visuals(tokens)
    } else {
        tokens.selection
    };

    ButtonVisuals {
        // If the non-selected button had a stroke, we should keep showing it.
        stroke: visuals.stroke,
        ..selected
    }
}

/// How big a button is, and where its fill sits relative to the box it was given.
struct Metrics {
    padding: Vec2,
    corner_radius: CornerRadius,
    min_size: Vec2,
    gap: f32,
    outer_margin: Margin,
}

fn metrics(args: &StyleArgs<'_>) -> Metrics {
    let explicit_size = args.inherited(Size::from_classes);
    let size = explicit_size.unwrap_or_default();

    let is_icon = args.classes.has_class(class::ICON);
    let is_custom = args.classes.has_class(Size::CLASS_CUSTOM);

    // In a popup, buttons are sized from its spacing the way egui sizes them, so they stack as
    // compact menu items instead of standalone buttons. `menu_style` tunes that spacing for menu
    // items and applies to every button under it; in other popups, only to buttons that didn't
    // ask for a size.
    let follows_spacing =
        crate::menu::is_menu_style(args.style) || (explicit_size.is_none() && in_popup(args));
    if follows_spacing && !is_icon && !is_custom {
        let spacing = &args.style.spacing;
        let min_height = explicit_size
            .map_or(0.0, Size::height)
            .max(spacing.interact_size.y);
        // egui paints a hovered widget's fill grown by its expansion without moving its layout,
        // so the fill bleeds that far past the button's box in every state.
        let expansion = args.style.visuals.widgets.hovered.expansion;
        return Metrics {
            padding: spacing.button_padding + Vec2::splat(expansion),
            corner_radius: args.style.visuals.widgets.inactive.corner_radius,
            min_size: Vec2::new(0.0, min_height),
            gap: spacing.icon_spacing,
            outer_margin: Margin::same(-(expansion as i8)),
        };
    }

    Metrics {
        padding: if is_icon { Vec2::ZERO } else { size.padding() },
        corner_radius: CornerRadius::same(args.style.tokens().normal_corner_radius()),
        min_size: if is_custom {
            Vec2::ZERO
        } else {
            size.icon_button_size()
        },
        gap: args.style.spacing.icon_spacing,
        outer_margin: Margin::ZERO,
    }
}

fn in_popup(args: &StyleArgs<'_>) -> bool {
    args.stack
        .iter()
        .any(|stack| matches!(stack.kind(), Some(UiKind::Popup | UiKind::Menu)))
}
