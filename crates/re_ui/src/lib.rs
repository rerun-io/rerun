//! Rerun GUI theme and helpers, built around [`egui`](https://www.egui.rs/).

mod command;
mod command_palette;
mod design_tokens;
pub mod egui_helpers;
pub mod icons;
mod static_image_cache;
pub mod toasts;
mod toggle_switch;

pub use command::Command;
pub use command_palette::CommandPalette;
pub use design_tokens::DesignTokens;
pub use icons::Icon;
pub use static_image_cache::StaticImageCache;
use std::ops::RangeInclusive;
pub use toggle_switch::toggle_switch;

// ---------------------------------------------------------------------------

/// If true, we fill the entire window, except for the close/maximize/minimize buttons in the top-left.
/// See <https://github.com/emilk/egui/pull/2049>
pub const FULLSIZE_CONTENT: bool = cfg!(target_os = "macos");

/// If true, we hide the native window decoration
/// (the top bar with app title, close button etc),
/// and instead paint our own close/maximize/minimize buttons.
pub const CUSTOM_WINDOW_DECORATIONS: bool = false; // !FULLSIZE_CONTENT; // TODO(emilk): https://github.com/rerun-io/rerun/issues/1063

/// If true, we show the native window decorations/chrome with the
/// close/maximize/minimize buttons and app title.
pub const NATIVE_WINDOW_BAR: bool = !FULLSIZE_CONTENT && !CUSTOM_WINDOW_DECORATIONS;

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

use egui::{pos2, Align2, Color32, Mesh, NumExt, Rect, Shape, Vec2};

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

    pub fn top_bar_margin() -> egui::Margin {
        egui::Margin::symmetric(8.0, 2.0)
    }

    /// Height of the top-most bar.
    pub fn top_bar_height() -> f32 {
        44.0 // from figma 2022-02-03
    }

    /// Height of the title row in the blueprint view and selection view,
    /// as well as the tab bar height in the viewport view.
    pub fn title_bar_height() -> f32 {
        28.0 // from figma 2022-02-03
    }

    pub fn native_window_rounding() -> f32 {
        10.0
    }

    #[inline]
    pub const fn box_width() -> f32 {
        139.0
    }

    #[inline]
    pub const fn box_height() -> f32 {
        22.0
    }

    pub fn labeled_combo_box<R>(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        selected_text: String,
        left_to_right: bool,
        menu_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) {
        let align = egui::Align::Center;
        let layout = if left_to_right {
            egui::Layout::left_to_right(align)
        } else {
            egui::Layout::right_to_left(align)
        };

        ui.with_layout(layout, |ui| {
            if left_to_right {
                ui.label(egui::RichText::new(label).color(self.design_tokens.gray_900));
            }
            ui.add_sized(
                [Self::box_width(), Self::box_height() + 1.0],
                |ui: &mut egui::Ui| {
                    egui::ComboBox::from_id_source(label)
                        .selected_text(selected_text)
                        .width(Self::box_width())
                        .show_ui(ui, menu_contents)
                        .response
                },
            );
            if !left_to_right {
                ui.label(egui::RichText::new(label).color(self.design_tokens.gray_900));
            }
        });
    }

    pub fn labeled_checkbox(&self, ui: &mut egui::Ui, label: &str, value: &mut bool) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(
                [Self::box_width(), Self::box_height()],
                |ui: &mut egui::Ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.checkbox(value, "");
                    })
                    .response
                },
            );
            ui.label(egui::RichText::new(label).color(self.design_tokens.gray_900));
        });
    }

    pub fn labeled_dragvalue<Num: egui::emath::Numeric>(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        value: &mut Num,
        range: RangeInclusive<Num>,
    ) where
        Num: egui::emath::Numeric,
    {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(
                [Self::box_width(), Self::box_height()],
                egui::DragValue::new(value).clamp_range(range),
            );
            ui.label(egui::RichText::new(label).color(self.design_tokens.gray_900));
        });
    }

    pub fn labeled_toggle_switch(&self, ui: &mut egui::Ui, label: &str, value: &mut bool) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(
                [Self::box_width(), Self::box_height()],
                |ui: &mut egui::Ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.add(toggle_switch(value));
                    })
                    .response
                },
            );
            ui.label(egui::RichText::new(label).color(self.design_tokens.gray_900));
        });
    }

    pub fn top_panel_frame(&self) -> egui::Frame {
        let mut frame = egui::Frame {
            inner_margin: Self::top_bar_margin(),
            fill: self.design_tokens.top_bar_color,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.rounding.nw = Self::native_window_rounding();
            frame.rounding.ne = Self::native_window_rounding();
        }
        frame
    }

    pub fn bottom_panel_margin(&self) -> egui::Vec2 {
        egui::Vec2::splat(8.0)
    }

    /// For the streams view (time panel)
    pub fn bottom_panel_frame(&self) -> egui::Frame {
        // Show a stroke only on the top. To achieve this, we add a negative outer margin.
        // (on the inner margin we counteract this again)
        let margin_offset = self.design_tokens.bottom_bar_stroke.width * 0.5;

        let margin = self.bottom_panel_margin();

        let mut frame = egui::Frame {
            fill: self.design_tokens.bottom_bar_color,
            inner_margin: egui::Margin::symmetric(
                margin.x + margin_offset,
                margin.y + margin_offset,
            ),
            outer_margin: egui::Margin {
                left: -margin_offset,
                right: -margin_offset,
                // Add a proper stoke width thick margin on the top.
                top: self.design_tokens.bottom_bar_stroke.width,
                bottom: -margin_offset,
            },
            stroke: self.design_tokens.bottom_bar_stroke,
            rounding: self.design_tokens.bottom_bar_rounding,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.rounding.sw = Self::native_window_rounding();
            frame.rounding.se = Self::native_window_rounding();
        }
        frame
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

    /// The color we use to mean "loop this selection"
    pub fn loop_selection_color() -> egui::Color32 {
        egui::Color32::from_rgb(1, 37, 105) // from figma 2023-02-09
    }

    /// The color we use to mean "loop all the data"
    pub fn loop_everything_color() -> egui::Color32 {
        egui::Color32::from_rgb(2, 80, 45) // from figma 2023-02-09
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
            // On mac we want to match the height of the native red/yellow/green close/minimize/maximize buttons.
            // TODO(emilk): move the native window buttons to match our Self::title_bar_height

            // Use more vertical space when zoomed in…
            let height = native_buttons_size_in_native_scale.y;

            // …but never shrink below the native button height when zoomed out.
            height.max(gui_zoom * native_buttons_size_in_native_scale.y)
        } else {
            Self::top_bar_height() - Self::top_bar_margin().sum().y
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
        let mut style = ui.style_mut().clone();
        style.spacing.button_padding = egui::Vec2::new(2.0, 2.0);
        ui.set_style(style);
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

    #[allow(clippy::unused_self)]
    pub fn large_collapsing_header<R>(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        default_open: bool,
        add_body: impl FnOnce(&mut egui::Ui) -> R,
    ) {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id(label),
            default_open,
        );

        let openness = state.openness(ui.ctx());

        let header_size = egui::vec2(ui.available_width(), 28.0);

        // Draw custom header.
        ui.allocate_ui_with_layout(
            header_size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let background_frame = ui.painter().add(egui::Shape::Noop);

                let space_before_icon = 0.0;
                let icon_width = ui.spacing().icon_width_inner;
                let space_after_icon = ui.spacing().icon_spacing;

                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let galley = ui.painter().layout_no_wrap(
                    label.to_owned(),
                    font_id,
                    Color32::TEMPORARY_COLOR,
                );

                let desired_size = header_size.at_least(
                    egui::vec2(space_before_icon + icon_width + space_after_icon, 0.0)
                        + galley.size(),
                );
                let header_response = ui.allocate_response(desired_size, egui::Sense::click());
                let rect = header_response.rect;

                let icon_rect = egui::Rect::from_center_size(
                    header_response.rect.left_center()
                        + egui::vec2(space_before_icon + icon_width / 2.0, 0.0),
                    egui::Vec2::splat(icon_width),
                );
                let icon_response = header_response.clone().with_new_rect(icon_rect);
                egui::collapsing_header::paint_default_icon(ui, openness, &icon_response);

                let visuals = ui.style().interact(&header_response);

                let text_pos = icon_response.rect.right_center()
                    + egui::vec2(space_after_icon, -0.5 * galley.size().y);
                ui.painter()
                    .galley_with_color(text_pos, galley, visuals.text_color());

                // Let the rect cover the full panel width:
                let bg_rect = rect.expand2(egui::vec2(1000.0, 0.0));
                ui.painter().set(
                    background_frame,
                    Shape::rect_filled(bg_rect, 0.0, visuals.bg_fill),
                );

                if header_response.clicked() {
                    state.toggle(ui);
                }
            },
        );
        state.show_body_unindented(ui, |ui| {
            ui.add_space(4.0); // Add space only if there is a body to make minimized headers stick together.
            add_body(ui);
            ui.add_space(4.0); // Same here
        });
    }

    /// Workaround for putting a label into a grid at the top left of its row.
    #[allow(clippy::unused_self)]
    pub fn grid_left_hand_label(&self, ui: &mut egui::Ui, label: &str) -> egui::Response {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.label(label)
        })
        .inner
    }

    /// Two-column grid to be used in selection view.
    #[allow(clippy::unused_self)]
    pub fn selection_grid(&self, ui: &mut egui::Ui, id: &str) -> egui::Grid {
        // Spread rows a bit to make it easier to see the groupings
        egui::Grid::new(id)
            .num_columns(2)
            .spacing(ui.style().spacing.item_spacing + egui::vec2(0.0, 8.0))
    }

    /// Draws a shadow into the given rect with the shadow direction given from dark to light
    #[allow(clippy::unused_self)]
    pub fn draw_shadow_line(&self, ui: &mut egui::Ui, rect: Rect, direction: egui::Direction) {
        let color_dark = self.design_tokens.shadow_gradient_dark_start;
        let color_bright = Color32::TRANSPARENT;

        let (left_top, right_top, left_bottom, right_bottom) = match direction {
            egui::Direction::RightToLeft => (color_bright, color_dark, color_bright, color_dark),
            egui::Direction::LeftToRight => (color_dark, color_bright, color_dark, color_bright),
            egui::Direction::BottomUp => (color_bright, color_bright, color_dark, color_dark),
            egui::Direction::TopDown => (color_dark, color_dark, color_bright, color_bright),
        };

        use egui::epaint::Vertex;
        let shadow = egui::Mesh {
            indices: vec![0, 1, 2, 2, 1, 3],
            vertices: vec![
                Vertex {
                    pos: rect.left_top(),
                    uv: egui::epaint::WHITE_UV,
                    color: left_top,
                },
                Vertex {
                    pos: rect.right_top(),
                    uv: egui::epaint::WHITE_UV,
                    color: right_top,
                },
                Vertex {
                    pos: rect.left_bottom(),
                    uv: egui::epaint::WHITE_UV,
                    color: left_bottom,
                },
                Vertex {
                    pos: rect.right_bottom(),
                    uv: egui::epaint::WHITE_UV,
                    color: right_bottom,
                },
            ],
            texture_id: Default::default(),
        };
        ui.painter().add(shadow);
    }

    pub fn selectable_label_with_icon(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        text: impl Into<egui::WidgetText>,
        selected: bool,
    ) -> egui::Response {
        let button_padding = ui.spacing().button_padding;
        let total_extra = button_padding + button_padding;

        let wrap_width = ui.available_width() - total_extra.x;
        let text = text
            .into()
            .into_galley(ui, None, wrap_width, egui::TextStyle::Button);

        let text_to_icon_padding = 4.0;
        let icon_width_plus_padding = Self::small_icon_size().x + text_to_icon_padding;

        let mut desired_size = total_extra + text.size() + egui::vec2(icon_width_plus_padding, 0.0);
        desired_size.y = desired_size
            .y
            .at_least(ui.spacing().interact_size.y)
            .at_least(Self::small_icon_size().y);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, selected, text.text())
        });

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, selected);

            // Draw background on interaction.
            if selected || response.hovered() || response.highlighted() || response.has_focus() {
                let rect = rect.expand(visuals.expansion);

                ui.painter().rect(
                    rect,
                    visuals.rounding,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                );
            }

            // Draw icon
            let image = self.icon_image(icon);
            let texture_id = image.texture_id(ui.ctx());
            // TODO(emilk/andreas): change color and size on hover
            let tint = ui.visuals().widgets.inactive.fg_stroke.color;
            let image_rect = egui::Rect::from_min_size(
                ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x.ceil(),
                    ((rect.min.y + rect.max.y - Self::small_icon_size().y) * 0.5).ceil(),
                )),
                Self::small_icon_size(),
            );
            ui.painter().image(
                texture_id,
                image_rect,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                tint,
            );

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x = image_rect.max.x + text_to_icon_padding;
            let text_pos = ui
                .layout()
                .align_size_within_rect(text.size(), text_rect)
                .min;
            text.paint_with_visuals(ui.painter(), text_pos, &visuals);
        }

        response
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "egui_dock")]
pub fn egui_dock_style(style: &egui::Style) -> egui_dock::Style {
    let mut dock_style = egui_dock::Style::from_egui(style);
    dock_style.separator_width = 2.0;
    dock_style.tab_bar_height = ReUi::title_bar_height();
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

    dock_style.hline_color = style.visuals.widgets.noninteractive.bg_stroke.color;

    // The active tab has no special outline:
    dock_style.tab_outline_color = Color32::TRANSPARENT;

    dock_style
}

// ----------------------------------------------------------------------------

/// Show some close/maximize/minimize buttons for the native window.
///
/// Assumes it is in a right-to-left layout.
///
/// Use when [`CUSTOM_WINDOW_DECORATIONS`] is set.
#[cfg(feature = "eframe")]
#[cfg(not(target_arch = "wasm32"))]
pub fn native_window_buttons_ui(frame: &mut eframe::Frame, ui: &mut egui::Ui) {
    use egui::{Button, RichText};

    let button_height = 12.0;

    let close_response = ui
        .add(Button::new(RichText::new("❌").size(button_height)))
        .on_hover_text("Close the window");
    if close_response.clicked() {
        frame.close();
    }

    if frame.info().window_info.maximized {
        let maximized_response = ui
            .add(Button::new(RichText::new("🗗").size(button_height)))
            .on_hover_text("Restore window");
        if maximized_response.clicked() {
            frame.set_maximized(false);
        }
    } else {
        let maximized_response = ui
            .add(Button::new(RichText::new("🗗").size(button_height)))
            .on_hover_text("Maximize window");
        if maximized_response.clicked() {
            frame.set_maximized(true);
        }
    }

    let minimized_response = ui
        .add(Button::new(RichText::new("🗕").size(button_height)))
        .on_hover_text("Minimize the window");
    if minimized_response.clicked() {
        frame.set_minimized(true);
    }
}
