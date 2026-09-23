//! A pulsing outline painted on top of the whole viewer, to point the user at something on
//! screen.
//!
//! Can be used by agents to point users to UI elements via `re_viewer_mcp`.

use egui::{FontId, Id, LayerId, Order, Rect, Stroke, StrokeKind, Ui};

use re_ui::UiExt as _;

/// Seconds per pulse of the outline.
const PULSE_PERIOD: f32 = 1.5;

/// Corner radius of the outline, matching the widget rounding it usually traces.
const CORNER_RADIUS: f32 = 4.0;

/// Gap between the outline and the label beside it.
const LABEL_GAP: f32 = 6.0;

/// Padding around the label text, inside its background.
const LABEL_PADDING: egui::Vec2 = egui::vec2(6.0, 3.0);

/// The layer the outline is painted on, above every panel and view.
const LAYER_ID: &str = "rerun_screen_highlight";

/// A rectangle the viewer outlines until the user clicks anywhere.
///
/// The rectangle is in logical points from the top-left of the viewer — the coordinate frame
/// `egui_mcp`'s `get_widget` reports widget bounds in.
pub struct ScreenHighlight {
    rect: Rect,
    label: Option<String>,

    /// `egui::InputState::time` when the highlight was installed.
    ///
    /// The pulse is phased from here rather than from viewer uptime, or the outline would start
    /// at whatever point of the cycle the viewer happened to be at.
    started: f64,
}

impl ScreenHighlight {
    /// `rect` is in logical points from the top-left of the viewer, and `started` is
    /// `egui::InputState::time` as of now.
    pub fn new(rect: Rect, label: Option<String>, started: f64) -> Self {
        Self {
            rect,
            label,
            started,
        }
    }

    /// Paint one frame of the pulse, and report whether the highlight should stay up.
    ///
    /// Returns `false` once the user has clicked anywhere, which is what dismisses it: the
    /// highlight is an interruption, so any interaction at all ends it.
    ///
    /// Call this *before* the viewer's panels, not after: the welcome screen resets the pointer
    /// state mid-frame, so a later read would never see the click. Painting goes to its own
    /// layer, so the outline still lands on top of everything.
    #[must_use]
    pub fn show(&self, ui: &Ui) -> bool {
        let ctx = ui.ctx();

        // Painted on its own foreground layer rather than into `ui`, so it lands on top of every
        // panel and view without taking part in any layout, and eats no clicks.
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique(LAYER_ID)));
        let tokens = ui.tokens();

        let (time, content_rect) = ctx.input(|i| (i.time, i.content_rect()));

        // A cosine starts each period at zero, so the outline grows out of the rectangle rather
        // than snapping to its widest on the first frame.
        let phase = (time - self.started) as f32 / PULSE_PERIOD;
        let pulse = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();

        let rect = self.rect.expand(egui::lerp(1.0..=6.0, pulse));
        let stroke = Stroke::new(egui::lerp(2.0..=4.0, pulse), tokens.selection_bg_fill);
        painter.rect_stroke(rect, CORNER_RADIUS, stroke, StrokeKind::Outside);

        if let Some(label) = &self.label {
            let font = FontId::proportional(14.0);
            let galley = painter.layout_no_wrap(label.clone(), font, tokens.text_color_on_primary);
            let size = galley.size() + LABEL_PADDING * 2.0;

            // Above the rect by default, below it when that would go off the top of the screen.
            let above = rect.top() - LABEL_GAP - size.y;
            let top = if content_rect.top() <= above {
                above
            } else {
                rect.bottom() + LABEL_GAP
            };

            // Centered on the rect, then pushed back inside the viewer: a widget near an edge
            // would otherwise have its label running off the screen, sideways most of all.
            // A label too big to fit stays flush with the top-left and overflows the far edge,
            // since the two bounds cross over and there is no position that satisfies both.
            let centered = egui::pos2(rect.center().x - 0.5 * size.x, top);
            let max = (content_rect.max - size).max(content_rect.min);
            let background = Rect::from_min_size(centered.clamp(content_rect.min, max), size);

            painter.rect_filled(background, CORNER_RADIUS, tokens.selection_bg_fill);
            painter.galley(
                background.min + LABEL_PADDING,
                galley,
                tokens.text_color_on_primary,
            );
        }

        // Nothing else animates while the viewer is idle, so the pulse has to keep itself alive.
        ctx.request_repaint();

        !ctx.input(|i| i.pointer.any_click())
    }
}
