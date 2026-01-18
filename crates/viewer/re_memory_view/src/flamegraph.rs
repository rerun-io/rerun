//! Flamegraph widget implementation.

use egui::remap_clamp;
use re_byte_size::{MemUsageTree, NamedMemUsageTree};

/// State for the flamegraph, stored in `ui.data_mut()`.
#[derive(Clone, Debug)]
pub struct FlamegraphState {
    /// UI points per byte (zoom level).
    pub zoom: f32,

    /// Horizontal pan offset in bytes.
    pub pan_bytes: f64,

    /// Whether to continuously auto-fit the view to the data.
    /// Disabled on first user input, re-enabled on double-click.
    auto_fit: bool,
}

impl Default for FlamegraphState {
    fn default() -> Self {
        Self {
            zoom: 1e-3,
            pan_bytes: 0.0,
            auto_fit: true,
        }
    }
}

impl FlamegraphState {
    /// Auto-fit the view to show all content.
    fn auto_fit(&mut self, total_size: u64, available_width: f32) {
        if 0 < total_size && 0.0 < available_width {
            // Calculate zoom to fit all content with some padding
            const PADDING_FACTOR: f32 = 0.95; // Leave 5% padding on sides
            self.zoom = (available_width * PADDING_FACTOR) / total_size as f32;
            self.pan_bytes = 0.0;
        }
    }
}

/// Render a flamegraph for the given memory usage tree.
pub fn flamegraph_ui(ui: &mut egui::Ui, tree: &NamedMemUsageTree, state: &mut FlamegraphState) {
    // Get total size for normalization
    let total_size = tree.value.size_bytes();
    if total_size == 0 {
        ui.label("No memory data available");
        return;
    }

    // Calculate available space
    let available_size = ui.available_size();
    let rect = ui.allocate_space(available_size).1;

    // Auto-fit continuously while enabled
    if state.auto_fit {
        state.auto_fit(total_size, rect.width());
    }

    // Check for double-click to re-enable auto-fit
    let response = ui.interact(rect, ui.id().with("flamegraph_area"), egui::Sense::click());
    if response.double_clicked() {
        state.auto_fit = true;
        state.auto_fit(total_size, rect.width());
    }

    // Handle zoom and pan input - only if mouse is hovering over the flamegraph
    let input = ui.input(|i| i.clone());
    let is_hovering = input
        .pointer
        .hover_pos()
        .is_some_and(|pos| rect.contains(pos));

    if is_hovering {
        // Handle zoom with scroll wheel (Ctrl/Cmd + scroll or pinch gesture)
        let zoom_factor = input.zoom_delta();
        if zoom_factor != 1.0 {
            // Get mouse position relative to the flamegraph
            if let Some(pointer_pos) = input.pointer.hover_pos() {
                // Calculate which byte offset the mouse is currently over (before zoom)
                let mouse_x_relative = pointer_pos.x - rect.min.x;
                let mouse_byte_pos = -(state.pan_bytes) + (mouse_x_relative / state.zoom) as f64;

                // Apply zoom
                state.zoom *= zoom_factor;
                state.zoom = state.zoom.clamp(1e-9, 1.0);

                // Adjust pan so that the same byte position remains under the mouse
                // After zoom: mouse_x_relative = (mouse_byte_pos - (-pan_bytes)) * new_zoom
                // Solving for new pan_bytes:
                state.pan_bytes = -(mouse_byte_pos - (mouse_x_relative / state.zoom) as f64);
            } else {
                // No mouse position, just zoom without adjusting pan
                state.zoom *= zoom_factor;
                state.zoom = state.zoom.clamp(1e-9, 1.0);
            }

            state.auto_fit = false; // Disable auto-fit on user zoom
        }

        // Handle pan with smooth scroll
        let scroll_delta = input.smooth_scroll_delta;
        if scroll_delta.x != 0.0 {
            // Convert UI points to bytes for panning
            state.pan_bytes += (scroll_delta.x / state.zoom) as f64;
            state.auto_fit = false; // Disable auto-fit on user pan
        }
    }

    // Render the flamegraph
    let x_start_bytes = -(state.pan_bytes);
    let x_end_bytes = x_start_bytes + (rect.width() / state.zoom) as f64;
    render_flamegraph_node(
        ui,
        &tree.value,
        &tree.name,
        state,
        rect,
        total_size,
        0.0,
        0.0,
        x_start_bytes,
        x_end_bytes,
    );
}

/// Recursively render a flamegraph node at a specific offset.
#[expect(clippy::too_many_arguments)]
fn render_flamegraph_node(
    ui: &mut egui::Ui,
    tree: &MemUsageTree,
    name: &str,
    state: &FlamegraphState,
    rect: egui::Rect,
    total_size: u64,
    depth: f32,
    x_offset_bytes: f64,
    x_start_bytes: f64,
    x_end_bytes: f64,
) {
    const ROW_HEIGHT: f32 = 20.0;
    const ROW_SPACING: f32 = 1.0;
    const TEXT_PADDING: f32 = 4.0;

    let size_bytes = tree.size_bytes();
    if size_bytes == 0 {
        return;
    }

    // Check if node is visible
    let node_end = x_offset_bytes + (size_bytes as f64);
    if node_end < x_start_bytes || x_end_bytes < x_offset_bytes {
        return;
    }

    // Convert bytes to UI coordinates
    let x_start_ui = rect.min.x + ((x_offset_bytes - x_start_bytes) as f32 * state.zoom);
    let x_end_ui =
        rect.min.x + ((x_offset_bytes + size_bytes as f64 - x_start_bytes) as f32 * state.zoom);
    let width_ui = x_end_ui - x_start_ui;

    // Calculate y position
    let y_pos = rect.min.y + depth * (ROW_HEIGHT + ROW_SPACING);

    if rect.max.y < y_pos {
        return;
    }

    let node_rect = egui::Rect::from_min_size(
        egui::pos2(x_start_ui, y_pos),
        egui::vec2(width_ui, ROW_HEIGHT),
    );

    // Only render if wide enough
    if 1.0 <= width_ui {
        let painter = ui.painter();

        // Draw background
        let color = generate_color(size_bytes as f32 / total_size as f32);
        painter.rect_filled(node_rect, 2.0, color);

        // Draw border
        painter.rect_stroke(
            node_rect,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::BLACK),
            egui::StrokeKind::Outside,
        );

        // Draw text if there's space
        if TEXT_PADDING * 2.0 < width_ui {
            let text = format!("{} {}", re_format::format_bytes(size_bytes as f64), name);

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

        // Add tooltip
        let id = ui
            .id()
            .with("child")
            .with(depth as u64)
            .with(x_offset_bytes as u64);
        let response = ui.interact(node_rect, id, egui::Sense::hover());
        response.on_hover_ui(|ui| {
            egui::Grid::new("flamegraph_tooltip_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.label(name);
                    ui.end_row();

                    ui.label("Size");
                    ui.label(re_format::format_bytes(size_bytes as f64));
                    ui.end_row();
                });
        });
    }

    // Render children recursively
    if let MemUsageTree::Node(node) = tree {
        let mut child_x_offset = x_offset_bytes;

        for child in node.children() {
            let child_size = child.value.size_bytes();
            if child_size == 0 {
                continue;
            }

            render_flamegraph_node(
                ui,
                &child.value,
                &child.name,
                state,
                rect,
                total_size,
                depth + 1.0,
                child_x_offset,
                x_start_bytes,
                x_end_bytes,
            );

            child_x_offset += child_size as f64;
        }
    }
}

/// Generate a color based on the fraction of total memory used.
/// Larger fractions get warmer colors (red), smaller fractions get cooler colors (blue/green).
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
