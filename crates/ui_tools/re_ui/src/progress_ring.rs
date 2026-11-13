//! The [`ProgressRing`] widget.

use std::f32::consts::TAU;

use crate::UiExt as _;

/// Width of the ring's line, in points: thick enough to read the fill at icon size.
const STROKE_WIDTH: f32 = 2.0;

/// A small ring, the size of an icon, that fills clockwise from twelve o'clock.
///
/// Shows how full something is in very little space, e.g. an agent's context window.
/// It shows no numbers, so pair it with a tooltip that does, and give it the same text as
/// [`Self::label`] so screen readers get it too.
pub struct ProgressRing {
    fraction: f32,
    label: Option<String>,
}

impl ProgressRing {
    /// `fraction` is how much of the ring is filled, clamped to `0..=1`. NaN counts as empty.
    pub fn new(fraction: f32) -> Self {
        let fraction = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        Self {
            fraction,
            label: None,
        }
    }

    /// The accessible name, e.g. what is filling up and by how much.
    ///
    /// Defaults to the fill as a percentage.
    #[inline]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl egui::Widget for ProgressRing {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { fraction, label } = self;

        let tokens = ui.tokens();
        let (rect, response) = ui.allocate_exact_size(tokens.small_icon_size, egui::Sense::hover());
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(
                egui::Role::ProgressIndicator,
                ui.is_enabled(),
                label
                    .clone()
                    .unwrap_or_else(|| format!("{:.0}%", 100.0 * fraction)),
            );
            info.value = Some(fraction.into());
            info
        });

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let center = rect.center();
            let radius = 0.5 * rect.size().min_elem() - 0.5 * STROKE_WIDTH;

            let track = ui.visuals().widgets.noninteractive.bg_stroke.color;
            painter.circle_stroke(center, radius, egui::Stroke::new(STROKE_WIDTH, track));

            let fill = egui::Stroke::new(STROKE_WIDTH, tokens.text_default);
            if fraction == 1.0 {
                // A closed circle, so there is no seam where the arc would start and end.
                painter.circle_stroke(center, radius, fill);
            } else if 0.0 < fraction {
                // About one segment per 6°, which looks round at any size we draw this at.
                let segments = ((60.0 * fraction).ceil() as usize).max(1);
                let points = (0..=segments)
                    .map(|i| {
                        let t = i as f32 / segments as f32;
                        center + radius * egui::Vec2::angled(TAU * (fraction * t - 0.25))
                    })
                    .collect();
                painter.add(egui::Shape::line(points, fill));
            }
        }

        response
    }
}
