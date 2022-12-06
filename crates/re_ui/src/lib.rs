mod design_tokens;
mod static_image_cache;

pub use design_tokens::DesignTokens;
pub use static_image_cache::StaticImageCache;

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
}
