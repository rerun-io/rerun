use egui::{emath::Float, pos2, Align2, Color32, Mesh, Rect, Shape, Vec2};

use crate::{DesignTokens, TopBarStyle};

/// Extension trait for [`egui::Context`].
///
/// This trait provides Rerun-specific helpers and utilities that require access to the egui
/// context.
pub trait ContextExt {
    fn ctx(&self) -> &egui::Context;

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

    #[must_use]
    fn warning_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.ctx().style();
        egui::RichText::new(text)
            .italics()
            .color(style.visuals.warn_fg_color)
    }

    #[must_use]
    fn error_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.ctx().style();
        egui::RichText::new(text)
            .italics()
            .color(style.visuals.error_fg_color)
    }

    fn top_bar_style(&self, style_like_web: bool) -> TopBarStyle {
        let egui_zoom_factor = self.ctx().zoom_factor();
        let fullscreen = self
            .ctx()
            .input(|i| i.viewport().fullscreen)
            .unwrap_or(false);

        // On Mac, we share the same space as the native red/yellow/green close/minimize/maximize buttons.
        // This means we need to make room for them.
        let make_room_for_window_buttons = !style_like_web && {
            #[cfg(target_os = "macos")]
            {
                crate::FULLSIZE_CONTENT && !fullscreen
            }
            #[cfg(not(target_os = "macos"))]
            {
                _ = fullscreen;
                false
            }
        };

        let native_buttons_size_in_native_scale = egui::vec2(64.0, 24.0); // source: I measured /emilk

        let height = if make_room_for_window_buttons {
            // On mac we want to match the height of the native red/yellow/green close/minimize/maximize buttons.
            // TODO(emilk): move the native window buttons to match our Self::title_bar_height

            // Use more vertical space when zoomed in…
            let height = native_buttons_size_in_native_scale.y;

            // …but never shrink below the native button height when zoomed out.
            height.max(native_buttons_size_in_native_scale.y / egui_zoom_factor)
        } else {
            DesignTokens::top_bar_height() - DesignTokens::top_bar_margin().sum().y
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
                .align_size_within_rect(texture.size, self.ctx().screen_rect())
                .translate(-Vec2::splat(16.0));
            let mut mesh = Mesh::with_texture(texture.id);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
            self.ctx().debug_painter().add(Shape::mesh(mesh));
        }
    }
}

impl ContextExt for egui::Context {
    fn ctx(&self) -> &egui::Context {
        self
    }
}
