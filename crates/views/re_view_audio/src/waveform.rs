use egui::emath::remap;
use egui::{Rangef, Sense};
use re_ui::UiExt as _;
use re_view::controls::DRAG_PAN2D_BUTTON;
use re_viewer_context::MOVE_TIME_CURSOR_ICON;

use crate::audio_asset_cache::DecodedAudio;

/// Up to this many visible frames, each pixel column scans the raw samples; beyond it,
/// columns read the cached envelope instead.
///
/// This trades accuracy for paint time. The raw scan is exact but costs one pass over every
/// visible frame on every repaint, while the envelope costs one lookup per column but only has
/// a fixed number of buckets over the whole clip, so it gets blocky when zoomed in.
/// At 48 kHz the limit is about 22 seconds of sample-level detail.
/// The two are not seamless: for clips longer than about 90 seconds, zoom levels just past this
/// limit show fewer envelope buckets than a 1000-pixel-wide view has columns.
const MAX_RAW_FRAMES_PER_PAINT: usize = 1 << 20;

/// Never zoom in past this many frames across the whole widget.
///
/// Past this point every column falls within the same few samples, so there is nothing more
/// to see, and the view range would approach zero width.
const MIN_VISIBLE_FRAMES: f64 = 64.0;

/// The part of the clip that is visible, as fractions of its duration.
#[derive(Clone, Copy, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct WaveformViewRange {
    pub min: f64,
    pub max: f64,
}

impl Default for WaveformViewRange {
    fn default() -> Self {
        Self { min: 0.0, max: 1.0 }
    }
}

impl WaveformViewRange {
    fn span(&self) -> f64 {
        self.max - self.min
    }

    fn is_everything(&self) -> bool {
        self.min <= 0.0 && 1.0 <= self.max
    }

    /// Shift by `delta` (in fractions of the duration), staying inside the clip.
    fn pan(&mut self, delta: f64) {
        let span = self.span();
        let min = (self.min + delta).clamp(0.0, 1.0 - span);
        *self = Self {
            min,
            max: min + span,
        };
    }

    /// Scale the visible span by `1 / zoom_factor` around `pivot` (a fraction of the duration).
    fn zoom(&mut self, pivot: f64, zoom_factor: f64, min_span: f64) {
        let new_span = (self.span() / zoom_factor).clamp(min_span, 1.0);
        let t = ((pivot - self.min) / self.span()).clamp(0.0, 1.0);
        let min = (pivot - t * new_span).clamp(0.0, 1.0 - new_span);
        *self = Self {
            min,
            max: min + new_span,
        };
    }
}

#[derive(Default)]
pub struct WaveformOutput {
    /// The user asked to move the time cursor to this offset into the clip, in seconds.
    pub seek_to_secs: Option<f64>,
}

/// Draws a zoomable waveform with the shared time cursor.
///
/// Interaction follows the time series view: primary drag pans, secondary click moves time,
/// dragging the cursor moves time, scrolling pans, zoom gestures zoom, and double-click resets.
pub fn waveform_ui(
    ui: &mut egui::Ui,
    decoded: &DecodedAudio,
    playhead_secs: Option<f64>,
    view_range: &mut WaveformViewRange,
    height: f32,
) -> WaveformOutput {
    re_tracing::profile_function!();

    let buffer = &decoded.buffer;
    let duration_secs = buffer.duration_secs();
    let num_frames = buffer.num_frames();

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    let mut output = WaveformOutput::default();
    if !ui.is_rect_visible(rect) || num_frames == 0 || duration_secs <= 0.0 {
        return output;
    }

    let t_from_x = |x: f32, range: &WaveformViewRange| -> f64 {
        remap(
            x as f64,
            rect.left() as f64..=rect.right() as f64,
            range.min..=range.max,
        )
    };
    let x_from_t = |t: f64, range: &WaveformViewRange| -> f32 {
        remap(
            t,
            range.min..=range.max,
            rect.left() as f64..=rect.right() as f64,
        ) as f32
    };

    // --- Pan & zoom, like the time panel and the plots:

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let hovered = pointer_pos.is_some_and(|p| rect.contains(p)) && ui.rect_contains_pointer(rect);

    let mut pan_delta_x = 0.0;
    let mut zoom_factor = 1.0;
    if hovered {
        ui.input(|input| {
            pan_delta_x += input.smooth_scroll_delta.x;
            zoom_factor *= input.zoom_delta_2d().x;
        });
    }

    let time_drag_id = response.id.with("time_drag");
    let is_dragging_time = ui.is_being_dragged(time_drag_id);

    if response.dragged_by(DRAG_PAN2D_BUTTON) && !is_dragging_time {
        pan_delta_x += response.drag_delta().x;
    }

    if pan_delta_x != 0.0 {
        let delta_t = -(pan_delta_x / rect.width()) as f64 * view_range.span();
        view_range.pan(delta_t);
    }

    if zoom_factor != 1.0
        && let Some(pointer_pos) = pointer_pos
    {
        let min_span = (MIN_VISIBLE_FRAMES / num_frames as f64).min(1.0);
        view_range.zoom(
            t_from_x(pointer_pos.x, view_range),
            zoom_factor as f64,
            min_span,
        );
    }

    if response.double_clicked() {
        *view_range = WaveformViewRange::default();
    }

    if view_range.is_everything() {
        *view_range = WaveformViewRange::default();
    }

    // --- Time cursor interaction:

    let cursor_x = playhead_secs
        .map(|secs| x_from_t(secs / duration_secs, view_range))
        .filter(|x| rect.x_range().contains(*x));

    let interact_radius = ui.style().interaction.resize_grab_radius_side;
    let is_near_cursor = cursor_x.is_some_and(|x| {
        let line_rect = egui::Rect::from_x_y_ranges(x..=x, rect.y_range()).expand(interact_radius);
        ui.rect_contains_pointer(line_rect)
    });

    if is_near_cursor || is_dragging_time {
        ui.ctx().set_cursor_icon(MOVE_TIME_CURSOR_ICON);
    }

    if is_near_cursor
        && !is_dragging_time
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
    {
        ui.set_dragged_id(time_drag_id);
    }

    let seek_to_x = if is_dragging_time || ui.is_being_dragged(time_drag_id) {
        pointer_pos.map(|p| p.x)
    } else if response.clicked() {
        response.interact_pointer_pos().map(|p| p.x)
    } else {
        None
    };
    if let Some(x) = seek_to_x {
        let t = t_from_x(x, view_range).clamp(0.0, 1.0);
        output.seek_to_secs = Some(t * duration_secs);
    }

    // --- Paint:

    let visuals = ui.visuals();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);

    paint_waveform(
        &painter,
        rect,
        decoded,
        view_range,
        visuals.widgets.inactive.fg_stroke.color,
    );

    // Hover preview, like the time panel:
    if hovered
        && !is_dragging_time
        && !is_near_cursor
        && let Some(pointer_pos) = pointer_pos
    {
        painter.vline(
            pointer_pos.x,
            rect.y_range(),
            visuals.widgets.noninteractive.fg_stroke,
        );
    }

    // The cursor is drawn where the pointer is while dragging, to avoid a frame of lag.
    let cursor_x = if is_dragging_time {
        pointer_pos.map(|p| p.x.clamp(rect.left(), rect.right()))
    } else if let Some(secs) = output.seek_to_secs {
        Some(x_from_t(secs / duration_secs, view_range))
    } else {
        cursor_x
    };
    if let Some(x) = cursor_x {
        let style = if is_dragging_time {
            &visuals.widgets.active
        } else if is_near_cursor {
            &visuals.widgets.hovered
        } else {
            &visuals.widgets.inactive
        };
        ui.paint_time_cursor_with_style(&painter, style, x, Rangef::new(rect.top(), rect.bottom()));
    }

    output
}

/// One vertical min/max line per pixel column of the visible range.
fn paint_waveform(
    painter: &egui::Painter,
    rect: egui::Rect,
    decoded: &DecodedAudio,
    view_range: &WaveformViewRange,
    color: egui::Color32,
) {
    re_tracing::profile_function!();

    let buffer = &decoded.buffer;
    let num_frames = buffer.num_frames();
    let center_y = rect.center().y;
    let half_height = (0.5 * rect.height() - 1.0).max(1.0);
    let num_columns = rect.width().floor().max(1.0) as usize;
    let visible_frames = (view_range.span() * num_frames as f64) as usize;
    let use_raw_samples = visible_frames <= MAX_RAW_FRAMES_PER_PAINT;

    let points = (0..num_columns)
        .filter_map(|column| {
            let t0 = remap(
                column as f64,
                0.0..=num_columns as f64,
                view_range.min..=view_range.max,
            );
            let t1 = remap(
                (column + 1) as f64,
                0.0..=num_columns as f64,
                view_range.min..=view_range.max,
            );

            let min_max = if use_raw_samples {
                let f0 = ((t0 * num_frames as f64) as usize).min(num_frames - 1);
                let f1 = ((t1 * num_frames as f64) as usize).clamp(f0 + 1, num_frames);
                mixed_min_max(buffer, f0..f1)
            } else {
                decoded.envelope.range(Rangef::new(t0 as f32, t1 as f32))
            }?;

            let x = rect.left() + column as f32 + 0.5;
            let y_top = center_y - min_max.max.clamp(-1.0, 1.0) * half_height;
            let y_bottom = center_y - min_max.min.clamp(-1.0, 1.0) * half_height;
            Some(egui::epaint::BandPoint::new(
                x,
                y_top.min(y_bottom - 1.0)..=y_bottom,
            ))
        })
        .collect();
    painter.add(egui::Shape::Band(egui::epaint::BandShape::filled(
        points, color,
    )));
}

/// Min and max of all channels mixed together over `frames`.
fn mixed_min_max(buffer: &re_audio::AudioBuffer, frames: std::ops::Range<usize>) -> Option<Rangef> {
    let num_channels = buffer.num_channels as usize;
    let samples = buffer
        .samples
        .get(frames.start * num_channels..frames.end * num_channels)?;
    samples
        .chunks_exact(num_channels)
        .map(|frame| frame.iter().sum::<f32>() / num_channels as f32)
        .map(Rangef::point)
        .reduce(Rangef::union)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_stays_inside_clip() {
        let mut range = WaveformViewRange { min: 0.2, max: 0.4 };
        range.pan(-0.5);
        assert_eq!(range, WaveformViewRange { min: 0.0, max: 0.2 });
        range.pan(2.0);
        assert_eq!(range, WaveformViewRange { min: 0.8, max: 1.0 });
    }

    #[test]
    fn zoom_keeps_pivot_and_clamps() {
        let mut range = WaveformViewRange::default();
        range.zoom(0.5, 2.0, 0.01);
        assert_eq!(
            range,
            WaveformViewRange {
                min: 0.25,
                max: 0.75
            }
        );

        range.zoom(0.25, 2.0, 0.01);
        assert!((range.min - 0.25).abs() < 1e-9, "{range:?}");
        assert!((range.max - 0.5).abs() < 1e-9, "{range:?}");

        range.zoom(0.3, 0.001, 0.01);
        assert_eq!(range, WaveformViewRange::default());

        range.zoom(0.5, 1e9, 0.01);
        assert!((range.span() - 0.01).abs() < 1e-9, "{range:?}");
    }
}
