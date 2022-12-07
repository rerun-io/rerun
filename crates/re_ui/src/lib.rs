//! Rerun GUI theme and helpers, built around [`egui`](https://www.egui.rs/).

mod design_tokens;
pub mod icons;
mod static_image_cache;

pub use design_tokens::DesignTokens;
pub use icons::Icon;
pub use static_image_cache::StaticImageCache;

// ---------------------------------------------------------------------------

/// If true, we fill the entire window, except for the close/maximize/minimize buttons in the top-left.
/// See <https://github.com/emilk/egui/pull/2049>
pub const FULLSIZE_CONTENT: bool = cfg!(target_os = "macos");

// ----------------------------------------------------------------------------

pub struct TopBarStyle {
    /// Height of the top bar
    pub height: f32,

    /// Extra horizontal space in the top left corner to make room for
    /// close/minimize/maximize buttons (on Mac)
    pub indent: f32,
}

// ----------------------------------------------------------------------------

use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub struct ReUi {
    pub egui_ctx: egui::Context,

    /// Colors, styles etc loaded from a design_tokens.json
    pub design_tokens: DesignTokens,

    pub static_image_cache: Arc<Mutex<StaticImageCache>>,
}

impl ReUi {
    /// Create [`ReUi`] and apply style to the given egui context.
    pub fn load_and_apply(egui_ctx: &egui::Context) -> Self {
        Self {
            egui_ctx: egui_ctx.clone(),
            design_tokens: DesignTokens::load_and_apply(egui_ctx),
            static_image_cache: Arc::new(Mutex::new(StaticImageCache::default())),
        }
    }

    pub fn rerun_logo(&self) -> Arc<egui_extras::RetainedImage> {
        if self.egui_ctx.style().visuals.dark_mode {
            self.static_image_cache.lock().get(
                "logo_dark_mode",
                include_bytes!("../data/logo_dark_mode.png"),
            )
        } else {
            self.static_image_cache.lock().get(
                "logo_light_mode",
                include_bytes!("../data/logo_light_mode.png"),
            )
        }
    }

    #[allow(clippy::unused_self)]
    pub fn panel_frame(&self) -> egui::Frame {
        let style = self.egui_ctx.style();
        egui::Frame {
            fill: style.visuals.window_fill(),
            inner_margin: egui::style::Margin::same(4.0),
            ..Default::default()
        }
    }

    #[allow(clippy::unused_self)]
    pub fn hovering_frame(&self) -> egui::Frame {
        let style = self.egui_ctx.style();
        egui::Frame {
            inner_margin: egui::style::Margin::same(2.0),
            outer_margin: egui::style::Margin::same(4.0),
            rounding: 4.0.into(),
            fill: style.visuals.window_fill(),
            stroke: style.visuals.window_stroke(),
            ..Default::default()
        }
    }

    #[allow(clippy::unused_self)]
    pub fn warning_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.egui_ctx.style();
        egui::RichText::new(text)
            .italics()
            .color(style.visuals.warn_fg_color)
    }

    pub fn loop_selection_color() -> egui::Color32 {
        egui::Color32::from_rgb(40, 200, 130)
    }

    /// Paint a watermark
    pub fn paint_watermark(&self) {
        use egui::*;
        let logo = self.rerun_logo();
        let screen_rect = self.egui_ctx.input().screen_rect;
        let size = logo.size_vec2();
        let rect = Align2::RIGHT_BOTTOM
            .align_size_within_rect(size, screen_rect)
            .translate(-Vec2::splat(16.0));
        let mut mesh = Mesh::with_texture(logo.texture_id(&self.egui_ctx));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
        self.egui_ctx.debug_painter().add(Shape::mesh(mesh));
    }

    pub fn top_bar_style(
        &self,
        native_pixels_per_point: Option<f32>,
        fullscreen: bool,
    ) -> TopBarStyle {
        let gui_zoom = if let Some(native_pixels_per_point) = native_pixels_per_point {
            native_pixels_per_point / self.egui_ctx.pixels_per_point()
        } else {
            1.0
        };

        // On Mac, we share the same space as the native red/yellow/green close/minimize/maximize buttons.
        // This means we need to make room for them.
        let make_room_for_window_buttons = {
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
            // Use more vertical space when zoomed in…
            let height = native_buttons_size_in_native_scale.y;

            // …but never shrink below the native button height when zoomed out.
            height.max(gui_zoom * native_buttons_size_in_native_scale.y)
        } else {
            self.egui_ctx.style().spacing.interact_size.y
        };

        let indent = if make_room_for_window_buttons {
            // Always use the same width measured in native GUI coordinates:
            gui_zoom * native_buttons_size_in_native_scale.x
        } else {
            0.0
        };

        TopBarStyle { height, indent }
    }

    pub fn medium_icon_toggle_button(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        selected: &mut bool,
    ) -> egui::Response {
        let size_points = egui::Vec2::splat(16.0); // TODO(emilk): get from design tokens

        let image = self.static_image_cache.lock().get(icon.id, icon.png_bytes);
        let texture_id = image.texture_id(ui.ctx());
        let tint = if *selected {
            ui.visuals().widgets.inactive.fg_stroke.color
        } else {
            egui::Color32::from_gray(100) // TODO(emilk): get from design tokens
        };
        let mut response = ui.add(egui::ImageButton::new(texture_id, size_points).tint(tint));
        if response.clicked() {
            *selected = !*selected;
            response.mark_changed();
        }
        response
    }
}
