//! Rerun GUI theme and helpers, built around [`egui`](https://www.egui.rs/).

mod command;
mod command_palette;
mod design_tokens;
mod layout_job_builder;
mod syntax_highlighting;
mod toggle_switch;

pub mod drag_and_drop;
pub mod icons;
pub mod list_item;
pub mod modal;
pub mod toasts;

pub use command::{UICommand, UICommandSender};
pub use command_palette::CommandPalette;
pub use design_tokens::DesignTokens;
pub use icons::Icon;
pub use layout_job_builder::LayoutJobBuilder;
pub use syntax_highlighting::SyntaxHighlighting;
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

/// The style of a label.
///
/// This should be used for all UI widgets that support these styles.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelStyle {
    /// Regular style for a label.
    #[default]
    Normal,

    /// Label displaying the placeholder text for a yet unnamed item (e.g. an unnamed space view).
    Unnamed,
}

// ----------------------------------------------------------------------------

use crate::list_item::ListItem;
use egui::emath::{Rangef, Rot2};
use egui::epaint::util::FloatOrd;
use egui::{pos2, Align2, CollapsingResponse, Color32, Mesh, NumExt, Rect, Shape, Vec2, Widget};

#[derive(Clone)]
pub struct ReUi {
    pub egui_ctx: egui::Context,

    /// Colors, styles etc loaded from a design_tokens.json
    pub design_tokens: DesignTokens,
}

impl ReUi {
    /// Create [`ReUi`] and apply style to the given egui context.
    pub fn load_and_apply(egui_ctx: &egui::Context) -> Self {
        egui_extras::install_image_loaders(egui_ctx);

        egui_ctx.include_bytes(
            "bytes://logo_dark_mode",
            include_bytes!("../data/logo_dark_mode.png"),
        );
        egui_ctx.include_bytes(
            "bytes://logo_light_mode",
            include_bytes!("../data/logo_light_mode.png"),
        );

        Self {
            egui_ctx: egui_ctx.clone(),
            design_tokens: DesignTokens::load_and_apply(egui_ctx),
        }
    }

    fn rerun_logo_uri(&self) -> &'static str {
        if self.egui_ctx.style().visuals.dark_mode {
            "bytes://logo_dark_mode"
        } else {
            "bytes://logo_light_mode"
        }
    }

    /// Welcome screen big title
    #[inline]
    pub fn welcome_screen_h1() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-h1".into())
    }

    #[inline]
    pub fn welcome_screen_h2() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-h2".into())
    }

    #[inline]
    pub fn welcome_screen_h3() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-h3".into())
    }

    #[inline]
    pub fn welcome_screen_example_title() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-example-title".into())
    }

    #[inline]
    pub fn welcome_screen_body() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-body".into())
    }

    pub fn welcome_screen_tab_bar_style(ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.visuals_mut().selection.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().selection.stroke = ui.visuals().widgets.active.fg_stroke;
        ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.hovered.fg_stroke = ui.visuals().widgets.active.fg_stroke;
        ui.visuals_mut().widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.inactive.fg_stroke = ui.visuals().widgets.noninteractive.fg_stroke;
    }

    /// Margin on all sides of views.
    pub fn view_padding() -> f32 {
        12.0
    }

    pub fn panel_margin() -> egui::Margin {
        egui::Margin::symmetric(Self::view_padding(), 0.0)
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
        16.0 // should be big enough to contain buttons, i.e. egui_style.spacing.interact_size.y
    }

    pub fn table_header_height() -> f32 {
        20.0
    }

    pub fn top_bar_margin() -> egui::Margin {
        egui::Margin::symmetric(8.0, 2.0)
    }

    pub fn text_to_icon_padding() -> f32 {
        4.0
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

    pub fn list_item_height() -> f32 {
        24.0
    }

    pub fn native_window_rounding() -> f32 {
        10.0
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

    #[allow(clippy::unused_self)]
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
        egui::Vec2::splat(14.0)
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
        if let Ok(egui::load::TexturePoll::Ready { texture }) = self.egui_ctx.try_load_texture(
            self.rerun_logo_uri(),
            egui::TextureOptions::default(),
            egui::SizeHint::Scale(1.0.ord()),
        ) {
            let rect = Align2::RIGHT_BOTTOM
                .align_size_within_rect(texture.size, self.egui_ctx.screen_rect())
                .translate(-Vec2::splat(16.0));
            let mut mesh = Mesh::with_texture(texture.id);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
            self.egui_ctx.debug_painter().add(Shape::mesh(mesh));
        }
    }

    pub fn top_bar_style(&self, style_like_web: bool) -> TopBarStyle {
        let egui_zoom_factor = self.egui_ctx.zoom_factor();
        let fullscreen = self
            .egui_ctx
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
            Self::top_bar_height() - Self::top_bar_margin().sum().y
        };

        let indent = if make_room_for_window_buttons {
            // Always use the same width measured in native GUI coordinates:
            native_buttons_size_in_native_scale.x / egui_zoom_factor
        } else {
            0.0
        };

        TopBarStyle { height, indent }
    }

    #[allow(clippy::unused_self)]
    pub fn small_icon_button(&self, ui: &mut egui::Ui, icon: &Icon) -> egui::Response {
        // TODO(emilk): change color and size on hover
        ui.add(
            egui::ImageButton::new(icon.as_image().fit_to_exact_size(Self::small_icon_size()))
                .tint(ui.visuals().widgets.inactive.fg_stroke.color),
        )
    }

    #[allow(clippy::unused_self)]
    pub fn medium_icon_toggle_button(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        selected: &mut bool,
    ) -> egui::Response {
        let size_points = egui::Vec2::splat(16.0); // TODO(emilk): get from design tokens

        let tint = if *selected {
            ui.visuals().widgets.inactive.fg_stroke.color
        } else {
            egui::Color32::from_gray(100) // TODO(emilk): get from design tokens
        };
        let mut response = ui
            .add(egui::ImageButton::new(icon.as_image().fit_to_exact_size(size_points)).tint(tint));
        if response.clicked() {
            *selected = !*selected;
            response.mark_changed();
        }
        response
    }

    #[allow(clippy::unused_self)]
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

            // no expansion effect
            visuals.widgets.hovered.expansion = 0.0;
            visuals.widgets.active.expansion = 0.0;
            visuals.widgets.open.expansion = 0.0;
        }

        let button_size = Vec2::splat(28.0);
        let icon_size = ReUi::small_icon_size(); // centered inside the button
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

            icon.as_image().tint(tint).paint_at(ui, image_rect);
        }

        ui.set_style(prev_style);

        response
    }

    #[allow(clippy::unused_self)]
    pub fn checkbox(
        &self,
        ui: &mut egui::Ui,
        selected: &mut bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.checkbox_indeterminate(ui, selected, text, false)
    }

    #[allow(clippy::unused_self)]
    pub fn checkbox_indeterminate(
        &self,
        ui: &mut egui::Ui,
        selected: &mut bool,
        text: impl Into<egui::WidgetText>,
        indeterminate: bool,
    ) -> egui::Response {
        ui.scope(|ui| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.visuals_mut().widgets.open.expansion = 0.0;

            // NOLINT
            egui::Checkbox::new(selected, text)
                .indeterminate(indeterminate)
                .ui(ui)
        })
        .inner
    }

    #[allow(clippy::unused_self)]
    pub fn radio_value<Value: PartialEq>(
        &self,
        ui: &mut egui::Ui,
        current_value: &mut Value,
        alternative: Value,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        ui.scope(|ui| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.visuals_mut().widgets.open.expansion = 0.0;

            // NOLINT
            ui.radio_value(current_value, alternative, text)
        })
        .inner
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

    /// Create a separator similar to [`egui::Separator`] but with the full span behavior.
    ///
    /// The span is determined by the current clip rectangle. Contrary to [`egui::Separator`], this separator allocates
    /// a single pixel in height, as spacing is typically handled by content when full span highlighting is used.
    pub fn full_span_separator(ui: &mut egui::Ui) -> egui::Response {
        let height = 1.0;

        let available_space = ui.available_size_before_wrap();
        let size = egui::vec2(available_space.x, height);

        let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());
        let clip_rect = ui.clip_rect();

        if ui.is_rect_visible(response.rect) {
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            let painter = ui.painter();

            painter.hline(
                clip_rect.left()..=clip_rect.right(),
                painter.round_to_pixel(rect.center().y),
                stroke,
            );
        }

        response
    }

    /// Popup similar to [`egui::popup_below_widget`] but suitable for use with
    /// [`crate::list_item::ListItem`].
    pub fn list_item_popup<R>(
        ui: &egui::Ui,
        popup_id: egui::Id,
        widget_response: &egui::Response,
        vertical_offset: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        if !ui.memory(|mem| mem.is_popup_open(popup_id)) {
            return None;
        }

        let pos = widget_response.rect.left_bottom() + egui::vec2(0.0, vertical_offset);
        let pivot = Align2::LEFT_TOP;

        let mut ret = None;
        egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .constrain(true)
            .fixed_pos(pos)
            .pivot(pivot)
            .show(ui.ctx(), |ui| {
                let frame = egui::Frame {
                    fill: ui.visuals().panel_fill,
                    ..Default::default()
                };
                let frame_margin = frame.total_margin();
                frame.show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        ui.set_width(widget_response.rect.width() - frame_margin.sum().x);

                        ui.set_clip_rect(ui.cursor());

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Frame {
                                //TODO(ab): use design token
                                inner_margin: egui::Margin::symmetric(8.0, 0.0),
                                ..Default::default()
                            }
                            .show(ui, |ui| ret = Some(add_contents(ui)))
                        })
                    })
                })
            });

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || widget_response.clicked_elsewhere() {
            ui.memory_mut(|mem| mem.close_popup());
        }
        ret
    }

    pub fn panel_content<R>(
        &self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&ReUi, &mut egui::Ui) -> R,
    ) -> R {
        egui::Frame {
            inner_margin: Self::panel_margin(),
            ..Default::default()
        }
        .show(ui, |ui| add_contents(self, ui))
        .inner
    }

    /// Static title bar used to separate panels into section.
    ///
    /// This title bar is meant to be used in a panel with proper inner margin and clip rectangle
    /// set.
    ///
    /// Use [`ReUi::panel_title_bar_with_buttons`] to display buttons in the title bar.
    pub fn panel_title_bar(&self, ui: &mut egui::Ui, label: &str, hover_text: Option<&str>) {
        self.panel_title_bar_with_buttons(ui, label, hover_text, |_ui| {});
    }

    /// Static title bar used to separate panels into section with custom buttons when hovered.
    ///
    /// This title bar is meant to be used in a panel with proper inner margin and clip rectangle
    /// set.
    #[allow(clippy::unused_self)]
    pub fn panel_title_bar_with_buttons<R>(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        hover_text: Option<&str>,
        add_right_buttons: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), Self::title_bar_height()),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // draw horizontal separator lines
                let mut rect = ui.available_rect_before_wrap();
                let hline_stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;
                rect.extend_with_x(ui.clip_rect().right());
                rect.extend_with_x(ui.clip_rect().left());
                ui.painter().hline(rect.x_range(), rect.top(), hline_stroke);
                ui.painter()
                    .hline(rect.x_range(), rect.bottom(), hline_stroke);

                // draw label
                let resp = ui.strong(label);
                if let Some(hover_text) = hover_text {
                    resp.on_hover_text(hover_text);
                }

                // draw hover buttons
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::right_to_left(egui::Align::Center),
                    add_right_buttons,
                )
                .inner
            },
        )
        .inner
    }

    /// Replacement for [`egui::CollapsingHeader`] that respect our style.
    ///
    /// The layout is fine-tuned to fit well in inspector panels (such as Rerun's Selection Panel)
    /// where the collapsing header should align nicely with checkboxes and other controls.
    #[allow(clippy::unused_self)]
    pub fn collapsing_header<R>(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        default_open: bool,
        add_body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::CollapsingResponse<R> {
        let id = ui.make_persistent_id(label);
        let button_padding = ui.spacing().button_padding;

        let available = ui.available_rect_before_wrap();
        // TODO(ab): use design token for indent — cannot use the global indent value as we must
        // align with checkbox, etc.
        let indent = 18.0;
        let text_pos = available.min + egui::vec2(indent, 0.0);
        let wrap_width = available.right() - text_pos.x;
        let wrap = Some(false);
        let galley = egui::WidgetText::from(label).into_galley(
            ui,
            wrap,
            wrap_width,
            egui::TextStyle::Button,
        );
        let text_max_x = text_pos.x + galley.size().x;

        let mut desired_width = text_max_x + button_padding.x - available.left();
        if ui.visuals().collapsing_header_frame {
            desired_width = desired_width.max(available.width()); // fill full width
        }

        let mut desired_size = egui::vec2(desired_width, galley.size().y + 2.0 * button_padding.y);
        desired_size = desired_size.at_least(ui.spacing().interact_size);
        let (_, rect) = ui.allocate_space(desired_size);

        let mut header_response = ui.interact(rect, id, egui::Sense::click());
        let text_pos = pos2(
            text_pos.x,
            header_response.rect.center().y - galley.size().y / 2.0,
        );

        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            default_open,
        );
        if header_response.clicked() {
            state.toggle(ui);
            header_response.mark_changed();
        }

        let openness = state.openness(ui.ctx());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&header_response);

            {
                let space_around_icon = 3.0;
                let icon_width = ui.spacing().icon_width_inner;

                let icon_rect = egui::Rect::from_center_size(
                    header_response.rect.left_center()
                        + egui::vec2(space_around_icon + icon_width / 2.0, 0.0),
                    egui::Vec2::splat(icon_width),
                );

                let icon_response = header_response.clone().with_new_rect(icon_rect);
                Self::paint_collapsing_triangle(
                    ui,
                    openness,
                    icon_rect.center(),
                    ui.style().interact(&icon_response),
                );
            }

            ui.painter().galley(text_pos, galley, visuals.text_color());
        }

        let ret_response = ui
            .vertical(|ui| {
                ui.spacing_mut().indent = indent;
                state.show_body_indented(&header_response, ui, add_body)
            })
            .inner;

        let (body_response, body_returned) =
            ret_response.map_or((None, None), |r| (Some(r.response), Some(r.inner)));

        CollapsingResponse {
            header_response,
            body_response,
            body_returned,
            openness,
        }
    }

    /// Conditionally collapsing header.
    ///
    /// Display content under a header that is conditionally collapsible. If `collapsing` is `true`,
    /// this is equivalent to [`ReUi::collapsing_header`]. If `collapsing` is `false`, the content
    /// is displayed under a static, non-collapsible header.
    #[allow(clippy::unused_self)]
    pub fn maybe_collapsing_header<R>(
        &self,
        ui: &mut egui::Ui,
        collapsing: bool,
        label: &str,
        default_open: bool,
        add_body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::CollapsingResponse<R> {
        if collapsing {
            self.collapsing_header(ui, label, default_open, add_body)
        } else {
            let response = ui.strong(label);
            CollapsingResponse {
                header_response: response,
                body_response: None,
                body_returned: None,
                openness: 1.0,
            }
        }
    }

    /// Show a prominent collapsing header to be used as section delimitation in side panels.
    ///
    /// Note that a clip rect must be set (typically by the panel) to avoid any overdraw.
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
                ui.visuals_mut().widgets.hovered.expansion = 0.0;
                ui.visuals_mut().widgets.active.expansion = 0.0;
                ui.visuals_mut().widgets.open.expansion = 0.0;

                let background_frame = ui.painter().add(egui::Shape::Noop);

                let space_before_icon = 0.0;
                let icon_width = ui.spacing().icon_width_inner;
                let space_after_icon = ui.spacing().icon_spacing;

                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let galley =
                    ui.painter()
                        .layout_no_wrap(label.to_owned(), font_id, Color32::PLACEHOLDER);

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
                Self::paint_collapsing_triangle(
                    ui,
                    openness,
                    icon_rect.center(),
                    ui.style().interact(&icon_response),
                );

                let visuals = ui.style().interact(&header_response);

                let optical_vertical_alignment = 0.5; // improves perceived vertical alignment
                let text_pos = icon_response.rect.right_center()
                    + egui::vec2(
                        space_after_icon,
                        -0.5 * galley.size().y + optical_vertical_alignment,
                    );
                ui.painter().galley(text_pos, galley, visuals.text_color());

                // Let the rect cover the full panel width:
                let mut bg_rect = rect;
                bg_rect.extend_with_x(ui.clip_rect().right());
                bg_rect.extend_with_x(ui.clip_rect().left());

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

    /// Layout area to allocate for the collapsing triangle.
    ///
    /// Note that this is not the _size_ of the collapsing triangle (which is defined by
    /// [`ReUi::paint_collapsing_triangle`]), but how much screen real-estate should be allocated
    /// for it. It's set to the same size as the small icon size so that everything is properly
    /// aligned in [`list_item::ListItem`].
    pub fn collapsing_triangle_area() -> egui::Vec2 {
        Self::small_icon_size()
    }

    /// Paint a collapsing triangle with rounded corners.
    ///
    /// Alternative to [`egui::collapsing_header::paint_default_icon`]. Note that the triangle is
    /// painted with a fixed size.
    pub fn paint_collapsing_triangle(
        ui: &egui::Ui,
        openness: f32,
        center: egui::Pos2,
        visuals: &egui::style::WidgetVisuals,
    ) {
        // This value is hard coded because, from a UI perspective, the size of the triangle is
        // given and fixed, and shouldn't vary based on the area it's in.
        static TRIANGLE_SIZE: f32 = 8.0;

        // Normalized in [0, 1]^2 space.
        // Note on how these coords have been computed: https://github.com/rerun-io/rerun/pull/2920
        // Discussion on the future of icons:  https://github.com/rerun-io/rerun/issues/2960
        let mut points = vec![
            pos2(0.80387, 0.470537),
            pos2(0.816074, 0.5),
            pos2(0.80387, 0.529463),
            pos2(0.316248, 1.017085),
            pos2(0.286141, 1.029362),
            pos2(0.257726, 1.017592),
            pos2(0.245118, 0.987622),
            pos2(0.245118, 0.012378),
            pos2(0.257726, -0.017592),
            pos2(0.286141, -0.029362),
            pos2(0.316248, -0.017085),
            pos2(0.80387, 0.470537),
        ];

        use std::f32::consts::TAU;
        let rotation = Rot2::from_angle(egui::remap(openness, 0.0..=1.0, 0.0..=TAU / 4.0));
        for p in &mut points {
            *p = center + rotation * (*p - pos2(0.5, 0.5)) * TRIANGLE_SIZE;
        }

        ui.painter().add(Shape::convex_polygon(
            points,
            visuals.fg_stroke.color,
            egui::Stroke::NONE,
        ));
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
    ///
    /// Use this when you expect the right column to have multi-line entries.
    #[allow(clippy::unused_self)]
    pub fn selection_grid(&self, _ui: &mut egui::Ui, id: &str) -> egui::Grid {
        // Spread rows a bit to make it easier to see the groupings
        let spacing = egui::vec2(8.0, 16.0);
        egui::Grid::new(id).num_columns(2).spacing(spacing)
    }

    /// Draws a shadow into the given rect with the shadow direction given from dark to light
    #[allow(clippy::unused_self)]
    pub fn draw_shadow_line(&self, ui: &egui::Ui, rect: Rect, direction: egui::Direction) {
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

    /// Convenience function to create a [`ListItem`] with the given text.
    pub fn list_item(&self, text: impl Into<egui::WidgetText>) -> ListItem<'_> {
        ListItem::new(self, text)
    }

    #[allow(clippy::unused_self)]
    pub fn selectable_label_with_icon(
        &self,
        ui: &mut egui::Ui,
        icon: &Icon,
        text: impl Into<egui::WidgetText>,
        selected: bool,
        style: LabelStyle,
    ) -> egui::Response {
        let button_padding = ui.spacing().button_padding;
        let total_extra = button_padding + button_padding;

        let wrap_width = ui.available_width() - total_extra.x;

        let mut text: egui::WidgetText = text.into();
        match style {
            LabelStyle::Normal => {}
            LabelStyle::Unnamed => {
                // TODO(ab): use design tokens
                text = text.italics();
            }
        }

        let galley = text.into_galley(ui, None, wrap_width, egui::TextStyle::Button);

        let icon_width_plus_padding = Self::small_icon_size().x + ReUi::text_to_icon_padding();

        let mut desired_size =
            total_extra + galley.size() + egui::vec2(icon_width_plus_padding, 0.0);
        desired_size.y = desired_size
            .y
            .at_least(ui.spacing().interact_size.y)
            .at_least(Self::small_icon_size().y);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, selected, galley.text())
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
            let image_size = Self::small_icon_size();
            let image_rect = egui::Rect::from_min_size(
                ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x.ceil(),
                    (rect.center().y - 0.5 * ReUi::small_icon_size().y).ceil(),
                )),
                image_size,
            );

            // TODO(emilk, andreas): change color and size on hover
            let tint = ui.visuals().widgets.inactive.fg_stroke.color;
            icon.as_image().tint(tint).paint_at(ui, image_rect);

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x = image_rect.max.x + ReUi::text_to_icon_padding();
            let text_pos = ui
                .layout()
                .align_size_within_rect(galley.size(), text_rect)
                .min;

            let mut text_color = visuals.text_color();
            match style {
                LabelStyle::Normal => {}
                LabelStyle::Unnamed => {
                    // TODO(ab): use design tokens
                    text_color = text_color.gamma_multiply(0.5);
                }
            }
            ui.painter()
                .galley_with_override_text_color(text_pos, galley, text_color);
        }

        response
    }

    /// Text format used for regular body.
    pub fn text_format_body(&self) -> egui::TextFormat {
        egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(&self.egui_ctx.style()),
            self.egui_ctx.style().visuals.text_color(),
        )
    }

    /// Text format used for labels referring to keys and buttons.
    pub fn text_format_key(&self) -> egui::TextFormat {
        let mut style = egui::TextFormat::simple(
            egui::TextStyle::Monospace.resolve(&self.egui_ctx.style()),
            self.egui_ctx.style().visuals.text_color(),
        );
        style.background = self.egui_ctx.style().visuals.widgets.noninteractive.bg_fill;
        style
    }

    /// Paints a time cursor for indicating the time on a time axis along x.
    #[allow(clippy::unused_self)]
    pub fn paint_time_cursor(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        response: &egui::Response,
        x: f32,
        y: Rangef,
    ) {
        let stroke = if response.dragged() {
            ui.style().visuals.widgets.active.fg_stroke
        } else if response.hovered() {
            ui.style().visuals.widgets.hovered.fg_stroke
        } else {
            ui.visuals().widgets.inactive.fg_stroke
        };

        let Rangef {
            min: y_min,
            max: y_max,
        } = y;

        let stroke = egui::Stroke {
            width: 1.5 * stroke.width,
            color: stroke.color,
        };

        let w = 10.0;
        let triangle = vec![
            pos2(x - 0.5 * w, y_min), // left top
            pos2(x + 0.5 * w, y_min), // right top
            pos2(x, y_min + w),       // bottom
        ];
        painter.add(egui::Shape::convex_polygon(
            triangle,
            stroke.color,
            egui::Stroke::NONE,
        ));
        painter.vline(x, (y_min + w)..=y_max, stroke);
    }
}

// ----------------------------------------------------------------------------

/// Show some close/maximize/minimize buttons for the native window.
///
/// Assumes it is in a right-to-left layout.
///
/// Use when [`CUSTOM_WINDOW_DECORATIONS`] is set.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_window_buttons_ui(ui: &mut egui::Ui) {
    use egui::{Button, RichText, ViewportCommand};

    let button_height = 12.0;

    let close_response = ui
        .add(Button::new(RichText::new("❌").size(button_height)))
        .on_hover_text("Close the window");
    if close_response.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }

    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        let maximized_response = ui
            .add(Button::new(RichText::new("🗗").size(button_height)))
            .on_hover_text("Restore window");
        if maximized_response.clicked() {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::Maximized(false));
        }
    } else {
        let maximized_response = ui
            .add(Button::new(RichText::new("🗗").size(button_height)))
            .on_hover_text("Maximize window");
        if maximized_response.clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(true));
        }
    }

    let minimized_response = ui
        .add(Button::new(RichText::new("🗕").size(button_height)))
        .on_hover_text("Minimize the window");
    if minimized_response.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

pub fn help_hover_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Label::new("❓").sense(egui::Sense::click()), // sensing clicks also gives hover effect
    )
}

/// Show some markdown
pub fn markdown_ui(ui: &mut egui::Ui, id: egui::Id, markdown: &str) {
    use parking_lot::Mutex;
    use std::sync::Arc;

    let commonmark_cache = ui.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<egui_commonmark::CommonMarkCache>>>(egui::Id::new(
            "global_egui_commonmark_cache",
        ))
        .clone()
    });

    egui_commonmark::CommonMarkViewer::new(id).show(ui, &mut commonmark_cache.lock(), markdown);
}
