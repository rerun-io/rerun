//! Flamegraph widget implementation.

use egui::{lerp, remap_clamp};
use re_byte_size::{MemUsageTree, NamedMemUsageTree};

/// Animation duration in seconds.
const ANIMATION_DURATION: f32 = 0.5;

/// View state: visible byte range and pan offset.
#[derive(Clone, Copy, Debug)]
struct View {
    /// How many bytes fit across the canvas width.
    canvas_width_bytes: f64,

    /// Horizontal pan offset in bytes.
    pan_bytes: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            canvas_width_bytes: 1000.0,
            pan_bytes: 0.0,
        }
    }
}

impl View {
    /// Create a view that shows a byte range with padding.
    fn from_range(start_bytes: f64, size_bytes: f64) -> Self {
        const PADDING: f64 = 0.01;
        let padded_size = size_bytes * (1.0 + 2.0 * PADDING);
        Self {
            canvas_width_bytes: padded_size,
            pan_bytes: start_bytes - size_bytes * PADDING,
        }
    }

    /// Linearly interpolate between two views.
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            canvas_width_bytes: lerp(a.canvas_width_bytes..=b.canvas_width_bytes, t as f64),
            pan_bytes: lerp(a.pan_bytes..=b.pan_bytes, t as f64),
        }
    }

    /// Returns the visible byte range as (start, end).
    fn visible_range(&self) -> (f64, f64) {
        let x_start = self.pan_bytes;
        let x_end = x_start + self.canvas_width_bytes;
        (x_start, x_end)
    }

    /// UI points per byte for the given canvas width.
    fn points_per_byte(&self, available_width: f32) -> f32 {
        available_width / self.canvas_width_bytes as f32
    }
}

/// State for the flamegraph, stored in `ui.data_mut()`.
#[derive(Clone, Debug)]
pub struct FlamegraphState {
    view: View,

    /// Whether to continuously auto-fit the view to the data.
    /// Disabled on first user input, re-enabled on double-click.
    auto_fit: bool,

    /// Animation state for smooth zoom transitions.
    animation: Option<ZoomAnimation>,
}

impl Default for FlamegraphState {
    fn default() -> Self {
        Self {
            view: View::default(),
            auto_fit: true,
            animation: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ZoomAnimation {
    start_time: f64,
    start: View,
    target: View,
}

impl FlamegraphState {
    /// Auto-fit the view to show all content.
    fn auto_fit(&mut self, total_size: u64) {
        if 0 < total_size {
            self.view = View::from_range(0.0, total_size as f64);
        }
    }

    /// Start a smooth animation to zoom to a specific byte range.
    fn animate_to_range(
        &mut self,
        current_time: f64,
        target_start_bytes: f64,
        target_size_bytes: f64,
    ) {
        if target_size_bytes <= 0.0 {
            return;
        }

        self.animation = Some(ZoomAnimation {
            start_time: current_time,
            start: self.view,
            target: View::from_range(target_start_bytes, target_size_bytes),
        });
    }

    /// Update animation state. Returns true if animation is in progress.
    fn update_animation(&mut self, current_time: f64) -> bool {
        let Some(anim) = &self.animation else {
            return false;
        };

        let elapsed = (current_time - anim.start_time) as f32;
        let t = (elapsed / ANIMATION_DURATION).clamp(0.0, 1.0);
        // Ease-out cubic for smooth deceleration
        let t_eased = 1.0 - (1.0 - t).powi(3);

        self.view = View::lerp(anim.start, anim.target, t_eased);

        if 1.0 <= t {
            self.animation = None;
            return false;
        }

        true
    }
}

/// Action requested by a flamegraph node on double-click.
struct ZoomToRange {
    start_bytes: f64,
    size_bytes: f64,
}

/// Context for rendering flamegraph nodes (constant throughout recursion).
struct RenderContext<'a> {
    /// UI points per byte (precomputed for efficiency).
    points_per_byte: f32,
    rect: egui::Rect,
    total_size_bytes: u64,
    x_start_bytes: f64,
    x_end_bytes: f64,
    zoom_action: &'a mut Option<ZoomToRange>,
}

/// Render a flamegraph for the given memory usage tree.
pub fn flamegraph_ui(ui: &mut egui::Ui, tree: &NamedMemUsageTree, state: &mut FlamegraphState) {
    // Get total size for normalization
    let total_size_bytes = tree.value.size_bytes();
    if total_size_bytes == 0 {
        ui.label("No memory data available");
        return;
    }

    // Calculate available space
    let available_size = ui.available_size();
    let rect = ui.allocate_space(available_size).1;

    // Update animation state
    let current_time = ui.input(|i| i.time);
    if state.update_animation(current_time) {
        ui.request_repaint();
    }

    // Auto-fit continuously while enabled (but not during animation)
    if state.auto_fit && state.animation.is_none() {
        state.auto_fit(total_size_bytes);
    }

    // Handle zoom and pan input - only if mouse is hovering over the flamegraph
    let input = ui.input(|i| i.clone());
    let hover_pos = input.pointer.hover_pos();
    let is_hovering = hover_pos.is_some_and(|pos| rect.contains(pos));

    if is_hovering {
        // Cancel animation on user interaction
        let has_interaction = input.zoom_delta() != 1.0 || input.smooth_scroll_delta.x != 0.0;
        if has_interaction {
            state.animation = None;
        }

        // Handle zoom with scroll wheel (Ctrl/Cmd + scroll or pinch gesture)
        let zoom_factor = input.zoom_delta();
        if zoom_factor != 1.0 {
            if let Some(pointer_pos) = hover_pos {
                let points_per_byte = state.view.points_per_byte(rect.width());
                let mouse_x_relative = pointer_pos.x - rect.min.x;
                let mouse_byte_pos =
                    state.view.pan_bytes + (mouse_x_relative / points_per_byte) as f64;

                // Zoom factor > 1 means zooming in, which means fewer bytes visible
                state.view.canvas_width_bytes /= zoom_factor as f64;
                state.view.canvas_width_bytes = state.view.canvas_width_bytes.clamp(1.0, 1e18);

                let new_points_per_byte = state.view.points_per_byte(rect.width());
                state.view.pan_bytes =
                    mouse_byte_pos - (mouse_x_relative / new_points_per_byte) as f64;
            } else {
                state.view.canvas_width_bytes /= zoom_factor as f64;
                state.view.canvas_width_bytes = state.view.canvas_width_bytes.clamp(1.0, 1e18);
            }

            state.auto_fit = false;
        }

        // Handle pan with smooth scroll
        let scroll_delta = input.smooth_scroll_delta;
        if scroll_delta.x != 0.0 {
            let points_per_byte = state.view.points_per_byte(rect.width());
            state.view.pan_bytes -= (scroll_delta.x / points_per_byte) as f64;
            state.auto_fit = false;
        }
    }

    // Render the flamegraph
    let (x_start_bytes, x_end_bytes) = state.view.visible_range();
    let mut zoom_action: Option<ZoomToRange> = None;

    let mut ctx = RenderContext {
        points_per_byte: state.view.points_per_byte(rect.width()),
        rect,
        total_size_bytes,
        x_start_bytes,
        x_end_bytes,
        zoom_action: &mut zoom_action,
    };

    let bg_response = ui.interact(rect, ui.id().with("flamegraph_bg"), egui::Sense::click());

    render_flamegraph_node(
        ui,
        tree,
        &mut ctx,
        0.0,
        0.0,
        ui.id().with("flamegraph_root"),
    );

    if let Some(action) = zoom_action {
        // Handle zoom action from double-click on a node:
        state.auto_fit = false;
        state.animate_to_range(current_time, action.start_bytes, action.size_bytes);
        ui.request_repaint();
    } else if bg_response.double_clicked() {
        // Reset view to show all content:
        state.auto_fit = true;
        state.animate_to_range(current_time, 0.0, total_size_bytes as f64);
        ui.request_repaint();
    }
}

/// Recursively render a flamegraph node at a specific offset.
fn render_flamegraph_node(
    ui: &mut egui::Ui,
    tree: &NamedMemUsageTree,
    ctx: &mut RenderContext<'_>,
    depth: f32,
    x_offset_bytes: f64,
    id: egui::Id,
) {
    const ROW_HEIGHT: f32 = 20.0;
    const ROW_SPACING: f32 = 1.0;
    const TEXT_PADDING: f32 = 4.0;
    const HOVER_LIGHTEN: f32 = 0.3;

    let size_bytes = tree.value.size_bytes();
    if size_bytes == 0 {
        return;
    }

    // Check if node is visible
    let node_end = x_offset_bytes + (size_bytes as f64);
    if node_end < ctx.x_start_bytes || ctx.x_end_bytes < x_offset_bytes {
        return;
    }

    // Convert bytes to UI coordinates
    let x_start_ui =
        ctx.rect.min.x + ((x_offset_bytes - ctx.x_start_bytes) as f32 * ctx.points_per_byte);
    let x_end_ui = ctx.rect.min.x
        + ((x_offset_bytes + size_bytes as f64 - ctx.x_start_bytes) as f32 * ctx.points_per_byte);
    let width_ui = x_end_ui - x_start_ui;

    // Calculate y position
    let y_pos = ctx.rect.min.y + depth * (ROW_HEIGHT + ROW_SPACING);

    if ctx.rect.max.y < y_pos {
        return;
    }

    let node_rect = egui::Rect::from_min_size(
        egui::pos2(x_start_ui, y_pos),
        egui::vec2(width_ui, ROW_HEIGHT),
    );

    // Only render if wide enough
    if 1.0 <= width_ui {
        let painter = ui.painter();

        // Handle interaction (hover tooltip + double-click zoom)
        let response = ui.interact(node_rect, id, egui::Sense::click());

        // Draw background with hover highlight
        let base_color = generate_color(size_bytes as f32 / ctx.total_size_bytes as f32);
        let color = if response.hovered() {
            lighten_color(base_color, HOVER_LIGHTEN)
        } else {
            base_color
        };
        painter.rect_filled(node_rect, 2.0, color);

        // Draw border
        let stroke = if response.hovered() {
            egui::Stroke::new(1.0, egui::Color32::WHITE)
        } else {
            egui::Stroke::new(1.0, egui::Color32::BLACK)
        };
        painter.rect_stroke(node_rect, 2.0, stroke, egui::StrokeKind::Outside);

        // Draw text if there's space
        if TEXT_PADDING * 2.0 < width_ui {
            let text = format!(
                "{} {}",
                re_format::format_bytes(size_bytes as f64),
                tree.name
            );

            let text_rect = node_rect.shrink(TEXT_PADDING);
            let text_color = if 384 < color.r() as u16 + color.g() as u16 + color.b() as u16 {
                egui::Color32::BLACK
            } else {
                egui::Color32::WHITE
            };

            painter.with_clip_rect(node_rect).text(
                text_rect.min,
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(12.0),
                text_color,
            );
        }

        // Double-click to zoom to this scope
        if response.double_clicked() {
            *ctx.zoom_action = Some(ZoomToRange {
                start_bytes: x_offset_bytes,
                size_bytes: size_bytes as f64,
            });
        }

        response.on_hover_ui(|ui| {
            egui::Grid::new("flamegraph_tooltip_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.label(&tree.name);
                    ui.end_row();

                    ui.label("Size");
                    ui.label(re_format::format_bytes(size_bytes as f64));
                    ui.end_row();
                });
        });
    }

    // Render children recursively
    if let MemUsageTree::Node(node) = &tree.value {
        let mut child_x_offset = x_offset_bytes;

        for child in node.children() {
            let child_id = id.with(&child.name);
            render_flamegraph_node(ui, child, ctx, depth + 1.0, child_x_offset, child_id);
            child_x_offset += child.size_bytes() as f64;
        }
    }
}

/// Generate a color based on the fraction of total memory used.
#[expect(
    clippy::disallowed_methods,
    reason = "Programmatic color generation for flamegraph visualization"
)]
fn generate_color(fraction: f32) -> egui::Color32 {
    // Brighter = more memory.
    // So we start with dark colors (blue) and later bright colors (green).
    let b = remap_clamp(fraction, 0.0..=0.15, 1.0..=0.3);
    let r = remap_clamp(fraction, 0.0..=0.30, 0.5..=0.8);
    let g = remap_clamp(fraction, 0.30..=1.0, 0.1..=0.8);
    let a = 0.9;
    (egui::Rgba::from_rgb(r, g, b) * a).into()
}

/// Lighten a color by blending it towards white.
#[expect(
    clippy::disallowed_methods,
    reason = "Programmatic color manipulation for flamegraph hover highlight"
)]
fn lighten_color(color: egui::Color32, amount: f32) -> egui::Color32 {
    let rgba = egui::Rgba::from(color);
    let lightened = egui::Rgba::from_rgb(
        lerp(rgba.r()..=1.0, amount),
        lerp(rgba.g()..=1.0, amount),
        lerp(rgba.b()..=1.0, amount),
    );
    lightened.into()
}
