use egui::emath::Float as _;
use egui::{Align2, Mesh, Rect, Shape, Vec2, pos2};

use crate::{DesignTokens, TopBarStyle};

/// Extension trait for [`egui::Context`].
///
/// This trait provides Rerun-specific helpers and utilities that require access to the egui
/// context.
pub trait ContextExt {
    fn ctx(&self) -> &egui::Context;

    fn tokens(&self) -> &'static DesignTokens {
        crate::design_tokens_of(self.ctx().theme())
    }

    /// Current time in seconds
    fn time(&self) -> f64 {
        self.ctx().input(|i| i.time)
    }

    // -----------------------------------------------------
    // Style-related stuff.
    // We could have this on a `StyleExt` trait, but we prefer to have it here on `Context`
    // so that it is the same style everywhere (instead of being specific to the parent `ui.style`).

    /// Text format used for regular body.
    fn text_format_body(&self) -> egui::TextFormat {
        egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(&self.ctx().style()),
            self.ctx().style().visuals.text_color(),
        )
    }

    /// Text format used for labels referring to keys and buttons.
    fn text_format_key(&self) -> egui::TextFormat {
        let mut style = egui::TextFormat::simple(
            egui::TextStyle::Monospace.resolve(&self.ctx().style()),
            self.ctx().style().visuals.text_color(),
        );
        style.background = self.ctx().style().visuals.widgets.noninteractive.bg_fill;
        style
    }

    fn rerun_logo_uri(&self) -> &'static str {
        if self.ctx().style().visuals.dark_mode {
            "bytes://logo_dark_mode"
        } else {
            "bytes://logo_light_mode"
        }
    }

    /// Hovered UI and spatial primitives should have this outline.
    fn hover_stroke(&self) -> egui::Stroke {
        // We want something bright here.
        self.ctx().style().visuals.widgets.active.fg_stroke
    }

    /// Selected UI and spatial primitives should have this outline.
    fn selection_stroke(&self) -> egui::Stroke {
        self.ctx().style().visuals.selection.stroke

        // It is tempting to use the background selection color for outlines,
        // but in practice it is way too dark for spatial views (you can't tell what is selected).
        // Also: background colors should not be used as stroke colors.
        // let color = self.ctx().style().visuals.selection.bg_fill;
        // let stroke_width = self.ctx().style().visuals.selection.stroke.width;
        // egui::Stroke::new(stroke_width, color)
    }

    /// Text colored to indicate success.
    #[must_use]
    fn success_text(&self, text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text).color(self.tokens().success_text_color)
    }

    /// Text colored to indicate a warning.
    ///
    /// For most cases, you should use [`crate::UiExt::warning_label`] instead,
    /// which has a nice fat border around it.
    #[must_use]
    fn warning_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.ctx().style();
        egui::RichText::new(text).color(style.visuals.warn_fg_color)
    }

    /// Text colored to indicate an error.
    ///
    /// For most cases, you should use [`crate::UiExt::error_label`] instead,
    /// which has a nice fat border around it.
    #[must_use]
    fn error_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.ctx().style();
        egui::RichText::new(text).color(style.visuals.error_fg_color)
    }

    fn top_bar_style(&self, _frame: &eframe::Frame, style_like_web: bool) -> TopBarStyle {
        let tokens = self.tokens();
        let egui_zoom_factor = self.ctx().zoom_factor();
        let fullscreen = self
            .ctx()
            .input(|i| i.viewport().fullscreen)
            .unwrap_or(false);

        // On Mac, we share the same space as the native red/yellow/green close/minimize/maximize buttons.
        // This means we need to make room for them.
        let make_room_for_window_buttons = !style_like_web
            && cfg!(target_os = "macos")
            && crate::fullsize_content(self.ctx().os())
            && !fullscreen;

        let traffic_button_sizes_fallback = egui::vec2(64.0, 24.0); // source: I measured /emilk

        #[cfg(target_os = "macos")]
        let native_buttons_size_in_native_scale = if make_room_for_window_buttons {
            use raw_window_handle::HasWindowHandle as _;

            use crate::egui_ext::WindowChromeMetrics;

            let metrics = _frame
                .window_handle()
                .ok()
                .and_then(|handle| WindowChromeMetrics::from_window_handle(&handle.as_raw()));
            if let Some(metrics) = metrics {
                let WindowChromeMetrics {
                    traffic_lights_size,
                } = metrics;
                traffic_lights_size
            } else {
                if cfg!(debug_assertions) {
                    re_log::warn_once!("Failed to measure the size of the mac traffic light area");
                }
                traffic_button_sizes_fallback
            }
        } else {
            egui::Vec2::ZERO
        };
        #[cfg(not(target_os = "macos"))]
        let native_buttons_size_in_native_scale = traffic_button_sizes_fallback;

        let height = if make_room_for_window_buttons {
            // On mac we want to match the height of the native red/yellow/green close/minimize/maximize buttons.
            // TODO(emilk): move the native window buttons to match our Self::title_bar_height

            // Use more vertical space when zoomed in…
            let height = native_buttons_size_in_native_scale.y;

            // …but never shrink below the native button height when zoomed out.
            height.max(native_buttons_size_in_native_scale.y / egui_zoom_factor)
        } else {
            tokens.top_bar_height() - tokens.top_bar_margin().sum().y
        };

        let indent = if make_room_for_window_buttons {
            // Always use the same width measured in native GUI coordinates:
            native_buttons_size_in_native_scale.x / egui_zoom_factor
        } else {
            0.0
        };

        TopBarStyle { height, indent }
    }

    /// Paint a watermark
    fn paint_watermark(&self) {
        if let Ok(egui::load::TexturePoll::Ready { texture }) = self.ctx().try_load_texture(
            self.rerun_logo_uri(),
            egui::TextureOptions::default(),
            egui::SizeHint::Scale(1.0.ord()),
        ) {
            let rect = Align2::RIGHT_BOTTOM
                .align_size_within_rect(texture.size, self.ctx().content_rect())
                .translate(-Vec2::splat(16.0));
            let mut mesh = Mesh::with_texture(texture.id);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            mesh.add_rect_with_uv(rect, uv, self.ctx().tokens().strong_fg_color);
            self.ctx().debug_painter().add(Shape::mesh(mesh));
        }
    }

    /// Whether to show extra information in the UI, e.g. in tooltips.
    ///
    /// This is controlled by holding the `Alt` key down.
    fn show_extras(&self) -> bool {
        self.ctx().input(|input| input.modifiers.alt)
    }
}

impl ContextExt for egui::Context {
    fn ctx(&self) -> &egui::Context {
        self
    }
}
