//! Rerun GUI theme and helpers, built around [`egui`](https://www.egui.rs/).

mod command;
mod command_palette;
mod design_tokens;
pub mod icons;
mod static_image_cache;
mod toggle_switch;

pub use command::Command;
pub use command_palette::CommandPalette;
pub use design_tokens::DesignTokens;
pub use icons::Icon;
pub use static_image_cache::StaticImageCache;
pub use toggle_switch::toggle_switch;

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

use std::sync::Arc;

use parking_lot::Mutex;

use egui::{pos2, Align2, Color32, Mesh, Rect, Shape, Vec2};

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

    /// Margin on all sides of views.
    pub fn view_padding() -> f32 {
        12.0
    }

    pub fn window_rounding() -> f32 {
        12.0
    }

    pub fn normal_rounding() -> f32 {
        6.0
    }

    pub fn small_rounding() -> f32 {
        4.0
    }

    pub fn table_line_height() -> f32 {
        14.0
    }

    pub fn table_header_height() -> f32 {
        20.0
    }

    /// Height of the title row in the blueprint view and selection view,
    /// as well as the tab bar height in the viewport view.
    pub fn top_bar_height() -> f32 {
        28.0 // from figma 2022-02-03
    }

    pub fn small_icon_size() -> egui::Vec2 {
        egui::Vec2::splat(12.0)
    }

    pub fn setup_table_header(_header: &mut egui_extras::TableRow<'_, '_>) {}

    pub fn setup_table_body(body: &mut egui_extras::TableBody<'_>) {
        // Make sure buttons don't visually overflow:
        body.ui_mut().spacing_mut().interact_size.y = Self::table_line_height();
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn warning_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.egui_ctx.style();
        egui::RichText::new(text)
            .italics()
            .color(style.visuals.warn_fg_color)
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn error_text(&self, text: impl Into<String>) -> egui::RichText {
        let style = self.egui_ctx.style();
        egui::RichText::new(text)
            .italics()
            .color(style.visuals.error_fg_color)
    }

    pub fn loop_selection_color() -> egui::Color32 {
        egui::Color32::from_rgb(30, 140, 90)
    }

    /// Paint a watermark
    pub fn paint_watermark(&self) {
        let logo = self.rerun_logo();
        let screen_rect = self.egui_ctx.screen_rect();
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

    pub fn icon_image(&self, icon: &Icon) -> Arc<egui_extras::RetainedImage> {
        self.static_image_cache.lock().get(icon.id, icon.png_bytes)
    }

    pub fn small_icon_button(&self, ui: &mut egui::Ui, icon: &Icon) -> egui::Response {
        let size_points = Self::small_icon_size();
        let image = self.icon_image(icon);
        let texture_id = image.texture_id(ui.ctx());
        // TODO(emilk): change color and size on hover
        let tint = ui.visuals().widgets.inactive.fg_stroke.color;
        ui.add(egui::ImageButton::new(texture_id, size_points).tint(tint))
    }

    pub fn medium_icon_toggle_button(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        selected: &mut bool,
    ) -> egui::Response {
        let size_points = egui::Vec2::splat(16.0); // TODO(emilk): get from design tokens

        let image = self.icon_image(icon);
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

    fn large_button_impl(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        bg_fill: Option<Color32>,
        tint: Option<Color32>,
    ) -> egui::Response {
        let prev_style = ui.style().clone();
        {
            // For big buttons we have a background color even when inactive:
            let visuals = ui.visuals_mut();
            visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
        }

        let image = self.icon_image(icon);
        let texture_id = image.texture_id(ui.ctx());

        let button_size = Vec2::splat(28.0);
        let icon_size = Vec2::splat(12.0); // centered inside the button
        let rounding = 6.0;

        let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ImageButton));

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let bg_fill = bg_fill.unwrap_or(visuals.bg_fill);
            let tint = tint.unwrap_or(visuals.fg_stroke.color);

            let image_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(icon_size, rect);
            // let image_rect = image_rect.expand2(expansion); // can make it blurry, so let's not

            ui.painter()
                .rect_filled(rect.expand(visuals.expansion), rounding, bg_fill);

            let mut mesh = egui::Mesh::with_texture(texture_id);
            let uv = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            mesh.add_rect_with_uv(image_rect, uv, tint);
            ui.painter().add(egui::Shape::mesh(mesh));
        }

        ui.set_style(prev_style);

        response
    }

    pub fn large_button(&self, ui: &mut egui::Ui, icon: &Icon) -> egui::Response {
        self.large_button_impl(ui, icon, None, None)
    }

    pub fn large_button_selected(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        selected: bool,
    ) -> egui::Response {
        let bg_fill = selected.then(|| ui.visuals().selection.bg_fill);
        let tint = selected.then(|| ui.visuals().selection.stroke.color);
        self.large_button_impl(ui, icon, bg_fill, tint)
    }

    pub fn visibility_toggle_button(
        &self,
        ui: &mut egui::Ui,
        visible: &mut bool,
    ) -> egui::Response {
        let mut response = if *visible && ui.is_enabled() {
            self.small_icon_button(ui, &icons::VISIBLE)
        } else {
            self.small_icon_button(ui, &icons::INVISIBLE)
        };
        if response.clicked() {
            response.mark_changed();
            *visible = !*visible;
        }
        response
    }

    /// Workaround for putting a label into a grid at the top left of its row.
    #[allow(clippy::unused_self)]
    pub fn grid_left_hand_label(&self, ui: &mut egui::Ui, label: &str) -> egui::Response {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.label(label)
        })
        .inner
    }

    /// Grid to be used in selection view.
    #[allow(clippy::unused_self)]
    pub fn selection_grid(&self, ui: &mut egui::Ui, id: &str) -> egui::Grid {
        // Spread rows a bit to make it easier to see the groupings
        egui::Grid::new(id).spacing(ui.style().spacing.item_spacing + egui::vec2(0.0, 8.0))
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "egui_dock")]
pub fn egui_dock_style(style: &egui::Style) -> egui_dock::Style {
    let mut dock_style = egui_dock::Style::from_egui(style);
    dock_style.separator_width = 2.0;
    dock_style.tab_bar_height = ReUi::top_bar_height();
    dock_style.default_inner_margin = 0.0.into();
    dock_style.show_close_buttons = false;
    dock_style.tab_include_scrollarea = false;
    dock_style.show_context_menu = false;
    dock_style.expand_tabs = false; // expand_tabs looks good, but decreases readability

    // Tabs can be "focused", meaning it was the last clicked (of any tab). We don't care about that.
    // Tabs can also be "active", meaning it is the selected tab within its sibling tabs. We want to highlight that.
    let inactive_text_color = style.visuals.widgets.noninteractive.text_color();
    let active_text_color = style.visuals.widgets.active.text_color();

    dock_style.tab_text_color_unfocused = inactive_text_color;
    dock_style.tab_text_color_focused = inactive_text_color;
    dock_style.tab_text_color_active_unfocused = active_text_color;
    dock_style.tab_text_color_active_focused = active_text_color;

    // Don't show tabs
    dock_style.tab_bar_background_color = style.visuals.panel_fill;
    dock_style.tab_background_color = style.visuals.panel_fill;

    // The active tab has no special outline:
    dock_style.tab_outline_color = Color32::TRANSPARENT;

    dock_style
}
