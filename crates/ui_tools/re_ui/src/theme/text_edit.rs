//! How [`egui::TextEdit`] is styled.

use egui::class::HasClasses as _;
use egui::theme::StyleProvider;
use egui::widget_style::{AtomLayoutStyle, StyleArgs, TextEditStyle, TextVisuals, WidgetState};
use egui::{Align2, CornerRadius, Frame, Margin, Stroke, TextStyle, Vec2};

use super::class::StyleArgsExt as _;
use super::{ReTheme, class};
use crate::HasDesignTokens as _;
use crate::button::Size;
use crate::text_edit::TextEditVariant;

/// The vertical padding of a text edit, which keeps its text off the frame edge.
///
/// The height comes from the size class instead; this only decides how tight a field is that has
/// more rows than its height allows for.
const PADDING_Y: i8 = 2;

impl StyleProvider<TextEditStyle> for ReTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> TextEditStyle {
        let StyleArgs { classes, style, .. } = args;

        let tokens = style.tokens();
        let variant = TextEditVariant::from_classes(classes).unwrap_or_default();
        let size = args.inherited(Size::from_classes).unwrap_or_default();
        let visuals = variant.visuals(tokens);

        // A focused text edit reads as `Active`; there is no separate pressed look.
        let (fill, mut stroke) = match args.state {
            WidgetState::Hovered => (visuals.fill_hovered, visuals.stroke_hovered),
            WidgetState::Active => (visuals.fill_focused, visuals.stroke_focused),
            WidgetState::Inactive | WidgetState::Noninteractive => (visuals.fill, visuals.stroke),
        };

        // Invalid content is outlined in the error color, whatever the state, so the field reads as
        // wrong whether or not the pointer happens to be over it.
        if classes.has_class(class::INVALID) {
            stroke = Stroke::new(1.0, style.visuals.error_fg_color);
        }

        // A text edit over an immutable buffer has no background, like egui's own text edit style:
        // callers that show a read-only value draw their own frame around it.
        let fill = if classes.has_class(egui::TextEdit::CLASS_READ_ONLY.as_str()) {
            egui::Color32::TRANSPARENT
        } else {
            fill
        };

        TextEditStyle {
            atom_layout: AtomLayoutStyle {
                // The same height a button of this size gets, so the two line up on a row.
                min_size: Vec2::new(0.0, size.height()),
                gap: style.spacing.icon_spacing,
                frame: Frame {
                    fill,
                    stroke,
                    corner_radius: CornerRadius::same(tokens.normal_corner_radius()),
                    // The stroke is painted centered on the frame edge, so half of it eats into
                    // the padding; compensate like the button does.
                    inner_margin: Margin::symmetric(size.padding().x as i8, PADDING_Y)
                        - Margin::same(stroke.width.round() as i8),
                    ..Default::default()
                },
                text_style: TextVisuals::new(style, TextStyle::Body, visuals.text),
                // Without this the text follows the alignment of the `Ui`, and centers in a
                // centered layout.
                align2: Some(Align2::LEFT_TOP),
                ..Default::default()
            },
            hint_text_color: visuals.text_placeholder,
            prefix_suffix_color: tokens.text_strong,
        }
    }
}
