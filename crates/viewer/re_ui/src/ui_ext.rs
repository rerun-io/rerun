use std::hash::Hash;

use egui::emath::{GuiRounding as _, Rot2};
use egui::{
    CollapsingResponse, Color32, IntoAtoms, NumExt as _, Rangef, Rect, StrokeKind, Widget as _,
    WidgetInfo, WidgetText, pos2,
};

use crate::alert::Alert;
use crate::button::ReButton;
use crate::list_item::{self, LabelContent};
use crate::{ContextExt as _, DesignTokens, Icon, LabelStyle, icons};

static FULL_SPAN_TAG: &str = "rerun_full_span";

fn error_label_bg_color(fg_color: Color32) -> Color32 {
    fg_color.gamma_multiply(0.35)
}

/// Rerun custom extensions to [`egui::Ui`].
pub trait UiExt {
    fn ui(&self) -> &egui::Ui;
    fn ui_mut(&mut self) -> &mut egui::Ui;

    fn theme(&self) -> egui::Theme {
        if self.ui().visuals().dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        }
    }

    fn tokens(&self) -> &'static DesignTokens {
        crate::design_tokens_of(self.theme())
    }

    /// Current time in seconds
    fn time(&self) -> f64 {
        self.ui().input(|i| i.time)
    }

    #[inline]
    #[track_caller]
    fn sanity_check(&self) {
        // TODO(emilk/egui#7537): add the contents of this function as a callback in egui instead.
        let ui = self.ui();

        if cfg!(debug_assertions)
            && ui.is_tooltip()
            && ui.spacing().tooltip_width + 1000.0 < ui.max_rect().width()
        {
            panic!("DEBUG ASSERT: Huge tooltip: {}", ui.max_rect().size());
        }
    }

    #[inline]
    fn is_tooltip(&self) -> bool {
        self.ui().layer_id().order == egui::Order::Tooltip
    }

    /// Shows a success label with a large border.
    ///
    /// If you don't want a border, use [`crate::ContextExt::success_text`].
    fn success_label(&mut self, success_text: impl Into<String>) -> egui::Response {
        Alert::success().show_text(self.ui_mut(), success_text.into(), None)
    }

    /// Shows a info label with a large border.
    fn info_label(&mut self, info_text: impl Into<String>) -> egui::Response {
        Alert::info().show_text(self.ui_mut(), info_text.into(), None)
    }

    /// Shows a warning label with a large border.
    ///
    /// If you don't want a border, use [`crate::ContextExt::warning_text`].
    fn warning_label(&mut self, warning_text: impl Into<String>) -> egui::Response {
        Alert::warning().show_text(self.ui_mut(), warning_text.into(), None)
    }

    /// Shows a small error label with the given text on hover and copies the text to the clipboard on click with a large border.
    ///
    /// This has a large border! If you don't want a border, use [`crate::ContextExt::error_text`].
    fn error_with_details_on_hover(&mut self, error_text: impl Into<String>) -> egui::Response {
        Alert::error().show_text(self.ui_mut(), "Error", Some(error_text.into()))
    }

    fn error_label_background_color(&self) -> egui::Color32 {
        error_label_bg_color(self.ui().style().visuals.error_fg_color)
    }

    /// Shows an error label with the entire error text and copies the text to the clipboard on click.
    ///
    /// Use this only if the error message is short, or you have a lot of room.
    /// Otherwise, use [`Self::error_with_details_on_hover`].
    ///
    /// This has a large border! If you don't want a border, use [`crate::ContextExt::error_text`].
    fn error_label(&mut self, error_text: impl Into<String>) -> egui::Response {
        Alert::error().show_text(self.ui_mut(), error_text.into(), None)
    }

    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    fn small_icon_button(&mut self, icon: &Icon, alt_text: impl Into<String>) -> egui::Response {
        let widget = self.small_icon_button_widget(icon, alt_text);
        self.ui_mut().add(widget)
    }

    /// The `alt_text` will be used for accessibility (e.g. read by screen readers),
    /// and is also how we can query the button in tests.
    fn small_icon_button_widget<'a>(
        &self,
        icon: &'a Icon,
        alt_text: impl Into<String>,
    ) -> egui::Button<'a> {
        egui::Button::image(
            icon.as_image()
                .fit_to_exact_size(self.tokens().small_icon_size)
                .alt_text(alt_text),
        )
        .image_tint_follows_text_color(true)
    }

    /// Adds a non-interactive, optionally tinted small icon.
    ///
    /// Uses [`DesignTokens::small_icon_size`]. Returns the rect where the icon was painted.
    fn small_icon(&mut self, icon: &Icon, tint: Option<egui::Color32>) -> egui::Rect {
        let ui = self.ui_mut();
        let (_, rect) = ui.allocate_space(ui.tokens().small_icon_size);
        let mut image = icon.as_image();
        if let Some(tint) = tint {
            image = image.tint(tint);
        }
        image.paint_at(ui, rect);

        rect
    }

    fn medium_icon_toggle_button(
        &mut self,
        icon: &Icon,
        alt_text: impl Into<String>,
        selected: &mut bool,
    ) -> egui::Response {
        let size_points = egui::Vec2::splat(16.0); // TODO(emilk): get from design tokens

        let tint = if *selected {
            self.ui().visuals().widgets.inactive.fg_stroke.color
        } else {
            self.ui().visuals().widgets.noninteractive.fg_stroke.color
        };
        let alt_text = alt_text.into();
        let mut response = self.ui_mut().add(egui::Button::new(
            icon.as_image()
                .fit_to_exact_size(size_points)
                .alt_text(alt_text.clone())
                .tint(tint),
        ));
        if response.clicked() {
            *selected = !*selected;
            response.mark_changed();
        }
        response.widget_info(|| {
            WidgetInfo::selected(egui::WidgetType::Button, true, *selected, alt_text.clone())
        });
        response
    }

    fn large_button_impl(
        &mut self,
        icon: &Icon,
        bg_fill: Option<Color32>,
        tint: Option<Color32>,
    ) -> egui::Response {
        let tokens = self.tokens();
        let button_size = tokens.large_button_size;
        let icon_size = tokens.large_button_icon_size; // centered inside the button
        let corner_radius = tokens.large_button_corner_radius;

        let ui = self.ui_mut();

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

        let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::Button));

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let bg_fill = bg_fill.unwrap_or(visuals.bg_fill);
            let tint = tint.unwrap_or(visuals.fg_stroke.color);

            let image_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(icon_size, rect);
            // let image_rect = image_rect.expand2(expansion); // can make it blurry, so let's not

            ui.painter()
                .rect_filled(rect.expand(visuals.expansion), corner_radius, bg_fill);

            icon.as_image().tint(tint).paint_at(ui, image_rect);
        }

        ui.set_style(prev_style);

        response
    }

    fn primary_button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> egui::Response {
        self.ui_mut().add(ReButton::new(atoms).primary())
    }

    fn secondary_button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> egui::Response {
        self.ui_mut().add(ReButton::new(atoms).secondary())
    }

    fn re_checkbox<'a>(
        &mut self,
        checked: &'a mut bool,
        text: impl IntoAtoms<'a>,
    ) -> egui::Response {
        self.checkbox_indeterminate(checked, text.into_atoms(), false)
    }

    #[expect(clippy::disallowed_types)]
    fn checkbox_indeterminate<'a>(
        &mut self,
        checked: &'a mut bool,
        text: impl IntoAtoms<'a>,
        indeterminate: bool,
    ) -> egui::Response {
        self.ui_mut()
            .scope(|ui| {
                ui.visuals_mut().widgets.hovered.expansion = 0.0;
                ui.visuals_mut().widgets.active.expansion = 0.0;
                ui.visuals_mut().widgets.open.expansion = 0.0;

                egui::Checkbox::new(checked, text)
                    .indeterminate(indeterminate)
                    .ui(ui)
            })
            .inner
    }

    #[expect(clippy::disallowed_methods)]
    fn re_radio_value<Value: PartialEq>(
        &mut self,
        current_value: &mut Value,
        alternative: Value,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.ui_mut()
            .scope(|ui| {
                ui.visuals_mut().widgets.hovered.expansion = 0.0;
                ui.visuals_mut().widgets.active.expansion = 0.0;
                ui.visuals_mut().widgets.open.expansion = 0.0;

                ui.radio_value(current_value, alternative, text)
            })
            .inner
    }

    fn large_button(&mut self, icon: &Icon) -> egui::Response {
        self.large_button_impl(icon, None, None)
    }

    fn large_button_selected(&mut self, icon: &Icon, selected: bool) -> egui::Response {
        let ui = self.ui();
        let bg_fill = selected.then(|| ui.visuals().selection.bg_fill);
        let tint = selected.then(|| ui.visuals().selection.stroke.color);
        self.large_button_impl(icon, bg_fill, tint)
    }

    fn visibility_toggle_button(&mut self, visible: &mut bool) -> egui::Response {
        let mut response = if *visible && self.ui().is_enabled() {
            self.small_icon_button(&icons::VISIBLE, "Make invisible")
        } else {
            self.small_icon_button(&icons::INVISIBLE, "Make visible")
        };
        if response.clicked() {
            response.mark_changed();
            *visible = !*visible;
        }
        response
    }

    /// Create a separator similar to [`egui::Separator`] but with the full span behavior.
    ///
    /// The span is determined using [`crate::UiExt::full_span`]. Contrary to
    /// [`egui::Separator`], this separator allocates a single pixel in height, as spacing is
    /// typically handled by content when full span highlighting is used.
    fn full_span_separator(&mut self) -> egui::Response {
        let ui = self.ui_mut();

        let height = 1.0;

        let available_space = ui.available_size_before_wrap();
        let size = egui::vec2(available_space.x, height);

        let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());

        if ui.is_rect_visible(response.rect) {
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            let painter = ui.painter();

            painter.hline(
                ui.full_span(),
                rect.center().y.round_to_pixels(painter.pixels_per_point()),
                stroke,
            );
        }

        response
    }

    /// Popup similar to [`egui::popup_below_widget`] but suitable for use with
    /// [`crate::list_item::ListItem`].
    ///
    /// Note that `add_contents` is called within a [`crate::list_item::list_item_scope`].
    fn list_item_popup<R>(
        &self,
        popup_id: egui::Id,
        widget_response: &egui::Response,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        let mut ret = None;

        egui::Popup::from_response(widget_response)
            .id(popup_id)
            .frame(egui::Frame::default())
            .open_memory(None)
            .gap(4.0)
            .layout(egui::Layout::top_down_justified(egui::Align::LEFT))
            .show(|ui| {
                ui.set_width(widget_response.rect.width());
                let frame = ui.tokens().popup_frame(ui.style());
                frame.show(ui, |ui| {
                    crate::list_item::list_item_scope(ui, popup_id, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ret = Some(add_contents(ui));
                        })
                    })
                })
            });

        ret
    }

    // TODO(ab): this used to be used for inner margin, after registering full span range in panels.
    // It's highly likely that all these use are now redundant.
    fn panel_content<R>(&mut self, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Frame {
            inner_margin: self.tokens().panel_margin(),
            ..Default::default()
        }
        .show(self.ui_mut(), |ui| add_contents(ui))
        .inner
    }

    /// Static title bar used to separate panels into section.
    ///
    /// This title bar is meant to be used in a panel with proper inner margin and clip rectangle
    /// set.
    ///
    /// Use [`UiExt::panel_title_bar_with_buttons`] to display buttons in the title bar.
    fn panel_title_bar(&mut self, label: &str, hover_text: Option<&str>) {
        self.panel_title_bar_with_buttons(label, hover_text, |_ui| {});
    }

    /// Static title bar used to separate panels into section with custom buttons when hovered.
    ///h
    /// This title bar is meant to be used in a panel with proper inner margin and clip rectangle
    /// set.
    fn panel_title_bar_with_buttons<R>(
        &mut self,
        label: &str,
        hover_text: Option<&str>,
        add_right_buttons: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let tokens = self.tokens();
        let ui = self.ui_mut();

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), tokens.title_bar_height()),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // draw horizontal separator lines
                let rect = egui::Rect::from_x_y_ranges(
                    ui.full_span(),
                    ui.available_rect_before_wrap().y_range(),
                );

                ui.painter()
                    .rect_filled(rect, 0.0, ui.tokens().section_header_color);

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
    fn collapsing_header<R>(
        &mut self,
        label: &str,
        default_open: bool,
        add_body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::CollapsingResponse<R> {
        let ui = self.ui_mut();
        let id = ui.make_persistent_id(label);
        let button_padding = ui.spacing().button_padding;

        let available = ui.available_rect_before_wrap();
        // TODO(ab): use design token for indent — cannot use the global indent value as we must
        // align with checkbox, etc.
        let indent = 18.0;
        let text_pos = available.min + egui::vec2(indent, 0.0);
        let wrap_width = available.right() - text_pos.x;
        let galley = egui::WidgetText::from(label).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
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
                ui.paint_collapsing_triangle(
                    openness,
                    icon_rect.center(),
                    ui.style().interact(&icon_response).fg_stroke.color,
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
    /// this is equivalent to [`Self::collapsing_header`]. If `collapsing` is `false`, the content
    /// is displayed under a static, non-collapsible header.
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    fn maybe_collapsing_header<R>(
        &mut self,

        collapsing: bool,
        label: &str,
        default_open: bool,
        add_body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::CollapsingResponse<R> {
        if collapsing {
            self.collapsing_header(label, default_open, add_body)
        } else {
            let response = self.ui_mut().strong(label);
            CollapsingResponse {
                header_response: response,
                body_response: None,
                body_returned: None,
                openness: 1.0,
            }
        }
    }

    /// Paint a collapsing triangle in the Rerun's style.
    ///
    /// Alternative to [`egui::collapsing_header::paint_default_icon`]. Note that the triangle is
    /// painted with a fixed size.
    fn paint_collapsing_triangle(&self, openness: f32, center: egui::Pos2, color: Color32) {
        // This value is hard coded because, from a UI perspective, the size of the triangle is
        // given and fixed, and shouldn't vary based on the area it's in.
        static TRIANGLE_SIZE: f32 = 8.0;

        // Normalized in [0, 1]^2 space.
        //
        // Note on how these coords were originally computed: https://github.com/rerun-io/rerun/pull/2920
        // Since then, the coordinates have been manually updated to Look Good(tm).
        //
        // Discussion on the future of icons: https://github.com/rerun-io/rerun/issues/2960
        let mut points = vec![
            pos2(0.306248, -0.017085), // top left end
            pos2(0.79387, 0.470537),   // ┐
            pos2(0.806074, 0.5),       // ├ "rounded" corner
            pos2(0.79387, 0.529463),   // ┘
            pos2(0.306248, 1.017085),  // bottom left end
        ];

        use std::f32::consts::TAU;
        let rotation = Rot2::from_angle(egui::remap(openness, 0.0..=1.0, 0.0..=TAU / 4.0));
        for p in &mut points {
            *p = center + rotation * (*p - pos2(0.5, 0.5)) * TRIANGLE_SIZE;
        }

        self.ui().painter().line(points, (1.0, color));
    }

    /// Workaround for putting a label into a grid at the top left of its row.
    ///
    /// You only need to use this if you expect the right side to have multi-line entries.
    fn grid_left_hand_label(&mut self, label: &str) -> egui::Response {
        self.ui_mut()
            .with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                ui.label(label)
            })
            .inner
    }

    /// Two-column grid to be used in selection view.
    ///
    /// Use this when you expect the right column to have multi-line entries.
    fn selection_grid(&self, id: &str) -> egui::Grid {
        // Spread rows a bit to make it easier to see the groupings
        let spacing = egui::vec2(8.0, 16.0);
        egui::Grid::new(id).num_columns(2).spacing(spacing)
    }

    /// Draws a shadow into the given rect with the shadow direction given from dark to light
    fn draw_shadow_line(&self, rect: Rect, direction: egui::Direction) {
        let color_dark = self.tokens().shadow_gradient_dark_start;
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
        self.ui().painter().add(shadow);
    }

    fn draw_focus_outline(&self, rect: Rect) {
        self.ui().painter().rect_stroke(
            rect,
            4,
            self.tokens().focus_outline_stroke,
            StrokeKind::Inside,
        );
        self.ui().painter().rect_stroke(
            rect,
            4,
            self.tokens().focus_halo_stroke,
            StrokeKind::Outside,
        );
    }

    /// Convenience function to create a [`list_item::list_item_scope`].
    #[inline]
    fn list_item_scope<R>(
        &mut self,
        id_salt: impl std::hash::Hash,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        list_item::list_item_scope(self.ui_mut(), id_salt, content)
    }

    /// Convenience function to create a [`list_item::ListItem`].
    fn list_item(&self) -> list_item::ListItem {
        list_item::ListItem::new()
    }

    fn list_item_label(&mut self, text: impl Into<WidgetText>) -> egui::Response {
        self.list_item()
            .interactive(false)
            .show_flat(self.ui_mut(), LabelContent::new(text))
    }

    /// Convenience for adding a flat non-interactive [`list_item::ListItemContent`]
    fn list_item_flat_noninteractive(
        &mut self,
        content: impl list_item::ListItemContent,
    ) -> egui::Response {
        self.list_item()
            .interactive(false)
            .show_flat(self.ui_mut(), content)
    }

    /// Convenience to create a non-interactive, collapsible [`list_item::ListItem`] with just a
    /// label. The children UI is wrapped in a [`list_item::list_item_scope`].
    fn list_item_collapsible_noninteractive_label<R>(
        &mut self,
        label: impl Into<egui::WidgetText>,
        default_open: bool,
        children_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        let label = label.into();
        let id = self.ui().id().with(egui::Id::new(label.text()));
        self.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                self.ui_mut(),
                id,
                default_open,
                list_item::LabelContent::new(label),
                |ui| list_item::list_item_scope(ui, id, children_ui).inner,
            )
            .body_response
            .map(|r| r.inner)
    }

    /// Convenience function to create a [`crate::SectionCollapsingHeader`].
    fn section_collapsing_header<'a>(
        &self,
        label: impl Into<egui::WidgetText>,
    ) -> crate::SectionCollapsingHeader<'a> {
        crate::SectionCollapsingHeader::new(label)
    }

    fn selectable_label_with_icon(
        &mut self,
        icon: &Icon,
        text: impl Into<egui::WidgetText>,
        selected: bool,
        style: LabelStyle,
    ) -> egui::Response {
        let ui = self.ui_mut();
        let tokens = ui.tokens();
        let button_padding = ui.spacing().button_padding;
        let total_extra = button_padding + button_padding;

        let available_rect = ui.available_rect_before_wrap();

        let view_rect = egui::Rect::from_min_max(
            available_rect.min,
            egui::pos2(
                available_rect
                    .max
                    .x
                    .min(ui.clip_rect().max.x - ui.spacing().window_margin.rightf()),
                available_rect.max.y,
            ),
        )
        .round_to_pixels(ui.pixels_per_point());

        let icon_width_plus_padding = tokens.small_icon_size.x + tokens.text_to_icon_padding();

        let wrap_width = view_rect.width() - icon_width_plus_padding - total_extra.x;

        let mut text: egui::WidgetText = text.into();
        let raw_text = text.text().to_owned();
        match style {
            LabelStyle::Normal => {}
            LabelStyle::Unnamed => {
                // TODO(ab): use design tokens
                text = text.italics();
            }
        }

        let galley = text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            wrap_width,
            egui::TextStyle::Button,
        );

        // 1 icons + padding.
        let mut desired_size =
            total_extra + galley.size() + egui::vec2(icon_width_plus_padding, 0.0);

        desired_size.y = desired_size
            .y
            .at_least(ui.spacing().interact_size.y)
            .at_least(tokens.small_icon_size.y);

        let show_copy_button = {
            /// The text character length at which the copy button will
            /// always be there. (unless the ui is disabled)
            const MIN_COPY_LEN: usize = 5;
            let enough_space = view_rect.width() > desired_size.x + icon_width_plus_padding;

            let long_enough_text = raw_text.chars().count() >= MIN_COPY_LEN;

            let id = ui.next_auto_id();
            let contains_pointer = ui.ctx().read_response(id).is_some_and(|last_response| {
                ui.rect_contains_pointer(
                    last_response
                        .interact_rect
                        .expand2(ui.spacing().item_spacing),
                )
            });

            ui.is_enabled() && (enough_space || long_enough_text) && contains_pointer
        };

        if show_copy_button {
            desired_size.x = (desired_size.x + icon_width_plus_padding).at_most(view_rect.width());
        }

        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                galley.text(),
            )
        });

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, selected);

            // Draw background on interaction.
            if selected || (response.hovered() || response.highlighted() || response.has_focus()) {
                let rect = rect.expand(visuals.expansion);

                ui.painter().rect(
                    rect,
                    visuals.corner_radius,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    egui::StrokeKind::Inside,
                );
            }

            // Draw icon
            let image_size = tokens.small_icon_size;
            let image_rect = egui::Rect::from_min_size(
                egui::pos2(
                    rect.min.x.ceil(),
                    (rect.center().y - 0.5 * tokens.small_icon_size.y).ceil(),
                )
                .round_to_pixels(ui.pixels_per_point()),
                image_size,
            );

            // TODO(emilk, andreas): change color and size on hover
            let icon_tint = if selected {
                if response.hovered() {
                    ui.tokens().icon_color_on_primary_hovered
                } else {
                    ui.tokens().icon_color_on_primary
                }
            } else {
                visuals.fg_stroke.color
            };
            icon.as_image().tint(icon_tint).paint_at(ui, image_rect);

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x = image_rect.max.x + tokens.text_to_icon_padding();
            let text_pos = egui::Align2([egui::Align::Min, ui.layout().vertical_align()])
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

            if show_copy_button {
                let copy_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.max.x - tokens.small_icon_size.x, image_rect.min.y)
                        .round_to_pixels(ui.pixels_per_point()),
                    tokens.small_icon_size,
                );

                let shape_idx = ui.painter().add(egui::Shape::Noop);
                let copy_response = ui.place(
                    copy_rect,
                    ui.small_icon_button_widget(&icons::COPY, "Copy")
                        .frame(false),
                );

                let copy_visuals = ui.style().interact(&copy_response);

                let color = if !copy_response.contains_pointer() {
                    visuals.weak_bg_fill
                } else {
                    copy_visuals.weak_bg_fill
                };

                ui.painter().set(
                    shape_idx,
                    egui::Shape::rect_filled(
                        copy_response.rect.expand(copy_visuals.expansion),
                        visuals.corner_radius,
                        color,
                    ),
                );

                if copy_response.clicked() {
                    re_log::info!("Copied {raw_text:?}");
                    ui.ctx().copy_text(raw_text);
                }
            }
        }

        response
    }

    fn loading_screen_ui<R>(&mut self, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ui = self.ui_mut();
        ui.set_min_height(ui.available_height());
        ui.center("loading spinner", |ui| {
            ui.vertical_centered(|ui| {
                ui.spinner();
                add_contents(ui)
            })
            .inner
        })
    }

    fn loading_screen(
        &mut self,
        header: impl Into<egui::RichText>,
        source: impl Into<egui::RichText>,
    ) {
        self.loading_screen_ui(|ui| {
            ui.label(
                header
                    .into()
                    .heading()
                    .color(ui.style().visuals.weak_text_color()),
            );
            ui.strong(source);
        });
    }

    /// Paints a time cursor for indicating the time on a time axis along x.
    fn paint_time_cursor(
        &self,
        painter: &egui::Painter,
        response: Option<&egui::Response>,
        x: f32,
        y: Rangef,
    ) {
        let ui = self.ui();
        let stroke = if let Some(response) = response {
            ui.visuals().widgets.style(response).fg_stroke
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

    /// Draw a bullet (for text lists).
    fn bullet(&mut self, color: Color32) {
        let ui = self.ui_mut();
        static DIAMETER: f32 = 6.0;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(DIAMETER, DIAMETER), egui::Sense::hover());

        ui.painter().add(egui::epaint::CircleShape {
            center: rect.center(),
            radius: DIAMETER / 2.0,
            fill: color,
            stroke: egui::Stroke::NONE,
        });
    }

    /// Center the content within [`egui::Ui::max_rect()`].
    ///
    /// The `add_contents` closure is executed in the context of a vertical layout.
    fn center<R>(
        &mut self,
        id_salt: impl Hash,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        // Strategy:
        // - estimate the size allocated by the `add_contents` closure
        // - add space based on the estimated size and `ui.max_size()`
        //
        // The estimation is done by recording the cursor position before and after the closure in
        // nested vertical/horizontal UIs such as for `ui.cursor()` to return the correct info.

        #[derive(Clone, Copy)]
        struct TextSize(egui::Vec2);

        let ui = self.ui_mut();
        let id = ui.make_persistent_id(id_salt);

        let text_size: Option<TextSize> = ui.data(|reader| reader.get_temp(id));

        // ensure the current ui has a vertical orientation so the space we add is in the correct
        // direction
        ui.vertical(|ui| {
            if let Some(text_size) = text_size {
                ui.add_space(ui.available_height() / 2.0 - text_size.0.y / 2.0);
            }

            ui.horizontal(|ui| {
                if let Some(text_size) = text_size {
                    ui.add_space(ui.available_width() / 2.0 - text_size.0.x / 2.0);
                }

                let starting_pos = ui.cursor().min;
                let (result, end_y) = ui
                    .vertical(|ui| (add_contents(ui), ui.cursor().min.y))
                    .inner;

                let end_pos = egui::pos2(ui.cursor().min.x, end_y);
                ui.data_mut(|writer| writer.insert_temp(id, TextSize(end_pos - starting_pos)));

                result
            })
            .inner
        })
        .inner
    }

    /// Binary toggle switch.
    ///
    /// Adapted from `egui_demo_lib/src/demo/toggle_switch.rs`
    fn toggle_switch(&mut self, height: f32, on: &mut bool) -> egui::Response {
        let ui = self.ui_mut();
        let width = (height / 2. * 3.).ceil();
        let size = egui::vec2(width, height); // 12x7 in figma, but 12x8 looks _much_ better in epaint

        let (interact_rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());

        let visual_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(size, interact_rect);

        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
        });

        if ui.is_rect_visible(visual_rect) {
            let how_on = ui.ctx().animate_bool(response.id, *on);
            let visuals = ui.style().interact(&response);
            let expanded_rect = visual_rect.expand(visuals.expansion);
            let fg_fill_off = visuals.bg_fill;
            let fg_fill_on = ui.visuals().selection.bg_fill;
            let fg_fill = fg_fill_off.lerp_to_gamma(fg_fill_on, how_on);
            let bg_fill_off = visuals.text_color();

            let corner_radius = 0.5 * expanded_rect.height();
            ui.painter()
                .rect_filled(expanded_rect, corner_radius, bg_fill_off);
            let circle_x = egui::lerp(
                (expanded_rect.left() + corner_radius)..=(expanded_rect.right() - corner_radius),
                how_on,
            );

            let circle_center = egui::pos2(circle_x, expanded_rect.center().y);
            let circle_radius_off = 0.3 * expanded_rect.height();
            let circle_radius_on = 0.35 * expanded_rect.height();
            ui.painter().circle_filled(
                circle_center,
                egui::lerp(circle_radius_off..=circle_radius_on, how_on),
                fg_fill,
            );
        }

        response
    }

    /// Helper for adding a list-item hyperlink.
    ///
    /// By default, the url is open in the same tab or a new tab based on the mouse button and
    /// modifiers, as per usual in browsers. If `always_new_tab` is `true`, then the url is opened
    /// in a new tab regardless.
    ///
    /// NOTE: for most kinds of URL, the `always_new_tab` is indirectly overridden to `true` by
    /// `re_viewer::app_state::check_for_clicked_hyperlinks()`, unless the URL is special-cased by
    /// that function (e.g. `rerun://` URLs).
    fn re_hyperlink(
        &mut self,
        text: impl Into<egui::WidgetText>,
        url: impl Into<String>,
        always_new_tab: bool,
    ) -> egui::Response {
        let ui = self.ui_mut();

        ui.scope(|ui| {
            let tokens = ui.tokens();
            let style = ui.style_mut();
            style.visuals.button_frame = false;

            let response = ui
                .add(crate::icons::EXTERNAL_LINK.as_button_with_label(tokens, text))
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if response.clicked_with_open_in_background() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url.into()));
            } else if response.clicked() {
                ui.ctx().open_url(egui::OpenUrl {
                    url: url.into(),
                    new_tab: always_new_tab || ui.input(|i| i.modifiers.any()),
                });
            }

            response
        })
        .inner
    }

    /// Show some close/maximize/minimize buttons for the native window.
    ///
    /// Assumes it is in a right-to-left layout.
    ///
    /// Use when [`crate::CUSTOM_WINDOW_DECORATIONS`] is set.
    #[cfg(not(target_arch = "wasm32"))]
    fn native_window_buttons_ui(&mut self) {
        use egui::{Button, RichText, ViewportCommand};

        let button_height = 12.0;

        let ui = self.ui_mut();

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

    /// Shows a `?` help button that will show a help UI when clicked.
    ///
    /// Until the user has interacted with a help button (any help button), we
    /// highlight the button extra to draw user attention to it.
    fn help_button(&mut self, help_ui: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        // The help menu appears when clicked and/or hovered
        let mut help_ui: Option<_> = Some(help_ui);

        let ui = self.ui_mut();

        // Have we ever shown any help UI anywhere?
        let has_shown_help_id = egui::Id::new("has_shown_help");
        let user_has_clicked_any_help_button: bool =
            ui.data_mut(|d| *d.get_persisted_mut_or_default(has_shown_help_id));

        // Draw attention to the help button by highlighting it when the user hovers
        // over its container (e.g. the tab bar of a view).
        let is_hovering_container = ui.rect_contains_pointer(ui.max_rect());

        ui.scope(|ui| {
            let where_to_paint_background =
                if !user_has_clicked_any_help_button && is_hovering_container {
                    Some(ui.painter().add(egui::Shape::Noop))
                } else {
                    None
                };

            let menu_button = egui::containers::menu::MenuButton::from_button(
                ui.small_icon_button_widget(&icons::HELP, "Help"),
            );

            let button_response = menu_button
                .ui(ui, |ui| {
                    if let Some(help_ui) = help_ui.take() {
                        help_ui(ui);
                        if !user_has_clicked_any_help_button {
                            // Remember that the user has found and used the help button at least once,
                            // to stop it from animating in the future:
                            ui.data_mut(|d| {
                                d.insert_persisted(has_shown_help_id, true);
                            });

                            #[cfg(feature = "analytics")]
                            re_analytics::record(|| re_analytics::event::HelpButtonFirstClicked {});
                        }
                    }
                })
                .0;

            if let Some(where_to_paint_background) = where_to_paint_background
                && !button_response.hovered()
            {
                let mut bg_rect = button_response.rect.expand(2.0);

                // Hack: ensure we don't paint outside the lines on the Y-axis.
                // Yes, we only do so for the Y axis, because if we do it for the X axis too
                // then the background won't be centered behind the help icon.
                bg_rect.min.y = bg_rect.min.y.max(ui.max_rect().min.y);
                bg_rect.max.y = bg_rect.max.y.min(ui.max_rect().max.y);

                ui.painter().set(
                    where_to_paint_background,
                    egui::Shape::rect_filled(bg_rect, 4.0, ui.tokens().highlight_color),
                );
            }

            if let Some(help_ui) = help_ui.take() {
                button_response.on_hover_ui(help_ui)
            } else {
                button_response
            }
        })
        .inner
    }

    /// Show some markdown
    fn markdown_ui(&mut self, markdown: &str) {
        use std::sync::Arc;

        use parking_lot::Mutex;

        let ui = self.ui_mut();
        let commonmark_cache = ui.data_mut(|data| {
            data.get_temp_mut_or_default::<Arc<Mutex<egui_commonmark::CommonMarkCache>>>(
                egui::Id::new("global_egui_commonmark_cache"),
            )
            .clone()
        });

        egui_commonmark::CommonMarkViewer::new().show(ui, &mut commonmark_cache.lock(), markdown);
    }

    /// A drop-down menu with a list of options.
    ///
    /// Designed for use with [`list_item`] content.
    ///
    /// Use this instead of using [`egui::ComboBox`] directly.
    fn drop_down_menu(
        &mut self,
        id_salt: impl std::hash::Hash,
        selected_text: String,
        content: impl FnOnce(&mut egui::Ui),
    ) -> egui::Response {
        // TODO(emilk): make the button itself a `ListItem2`
        let response = egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected_text.clone())
            .show_ui(self.ui_mut(), |ui| {
                list_item::list_item_scope(ui, "inner_scope", |ui| {
                    content(ui);
                });
            });
        response.response
    }

    /// Use the provided range as full span for the nested content.
    ///
    /// See [`Self::full_span`] for details.
    fn full_span_scope<R>(
        &mut self,
        span: impl Into<egui::Rangef>,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        self.ui_mut()
            .scope_builder(
                egui::UiBuilder::new().ui_stack_info(
                    egui::UiStackInfo::default().with_tag_value(FULL_SPAN_TAG, span.into()),
                ),
                content,
            )
            .inner
    }

    /// Retrieve the current full-span scope.
    ///
    /// By default, this method uses a heuristics to identify which parent `Ui`'s boundary should be
    /// used (e.g. top-level panel, tooltip, etc.). Use [`Self::full_span_scope`] to set a specific
    /// range as full span.
    fn full_span(&self) -> egui::Rangef {
        for node in self.ui().stack().iter() {
            if let Some(span) = node.tags().get_downcast(FULL_SPAN_TAG) {
                return *span;
            }

            if node.has_visible_frame()
                || node.is_area_ui()
                || node.is_panel_ui()
                || node.is_root_ui()
            {
                return (node.max_rect + node.frame().inner_margin).x_range();
            }
        }

        // should never happen
        egui::Rangef::EVERYTHING
    }

    /// Style [`egui::Ui::selectable_value`]s and friends into a horizontal, toggle-like widget.
    ///
    /// # Example
    ///
    /// ```
    /// # egui::__run_test_ui(|ui| {
    /// # use re_ui::UiExt as _;
    /// let mut flag = false;
    /// ui.selectable_toggle(|ui| {
    ///     ui.selectable_value(&mut flag, false, "Inactive");
    ///     ui.selectable_value(&mut flag, true, "Active");
    /// });
    /// # });
    /// ```
    fn selectable_toggle<R>(
        &mut self,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let ui = self.ui_mut();

        let tokens = ui.tokens();
        egui::Frame {
            inner_margin: egui::Margin::same(3),
            stroke: tokens.bottom_bar_stroke,
            corner_radius: ui.visuals().widgets.hovered.corner_radius + egui::CornerRadius::same(3),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.visuals_mut().widgets.inactive.expansion = 0.0;

            ui.visuals_mut().selection.bg_fill = ui.visuals_mut().widgets.inactive.bg_fill;
            ui.visuals_mut().selection.stroke = ui.visuals_mut().widgets.inactive.fg_stroke;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;

            ui.visuals_mut().widgets.hovered.fg_stroke.color =
                ui.visuals().widgets.inactive.fg_stroke.color;
            ui.visuals_mut().widgets.active.fg_stroke.color =
                ui.visuals().widgets.inactive.fg_stroke.color;
            ui.visuals_mut().widgets.inactive.fg_stroke.color =
                ui.visuals().widgets.noninteractive.fg_stroke.color;

            ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
            ui.spacing_mut().item_spacing.x = 3.0;

            ui.horizontal(content).inner
        })
    }

    /// Set [`egui::Style::wrap_mode`] to [`egui::TextWrapMode::Truncate`], unless this is a sizing
    /// pass, in which case it is set to [`egui::TextWrapMode::Extend`].
    fn set_truncate_style(&mut self) {
        let ui = self.ui_mut();
        if ui.is_sizing_pass() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        } else {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }
    }

    /// Display some UI that may optionally include extras (see [`crate::ContextExt::show_extras`]).
    ///
    /// This assumes that the content will change based on whether extras are shown or not, so it
    /// takes care of triggering a sizing pass and repaint as required.
    ///
    /// The closure is passed a `bool` indicating whether extras are shown or not.
    fn with_optional_extras<R>(&mut self, content: impl FnOnce(&mut egui::Ui, bool) -> R) -> R {
        let ui = self.ui_mut();

        let show_extras = ui.ctx().show_extras();

        let content_changed = ui.data_mut(|data| {
            let stored_show_extras = data
                .get_temp_mut_or_insert_with(ui.id().with("__stored_show_extra__"), || show_extras);
            if *stored_show_extras != show_extras {
                *stored_show_extras = show_extras;
                true
            } else {
                false
            }
        });

        let mut builder = egui::UiBuilder::new();
        if content_changed {
            builder = builder.sizing_pass();
            ui.ctx().request_repaint();
        }

        ui.scope_builder(builder, |ui| content(ui, show_extras))
            .inner
    }

    /// Menu item with an icon and text.
    fn icon_and_text_menu_item(
        &mut self,
        icon: &Icon,
        text: impl Into<WidgetText>,
    ) -> egui::Response {
        let ui = self.ui_mut();
        let tokens = ui.tokens();
        ui.add(egui::Button::image_and_text(
            icon.as_image()
                .tint(tokens.label_button_icon_color)
                .fit_to_exact_size(tokens.small_icon_size),
            text,
        ))
    }

    /// Set the current style for a text field that has invalid content.
    fn style_invalid_field(&mut self) {
        let ui = self.ui_mut();
        ui.visuals_mut().selection.stroke.color = ui.visuals().error_fg_color;
        ui.visuals_mut().widgets.active.bg_stroke =
            egui::Stroke::new(1.0, ui.visuals().error_fg_color);
        ui.visuals_mut().widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, ui.visuals().error_fg_color);
        ui.visuals_mut().widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    }
}

impl UiExt for egui::Ui {
    #[inline]
    fn ui(&self) -> &egui::Ui {
        self
    }

    #[inline]
    fn ui_mut(&mut self) -> &mut egui::Ui {
        self
    }
}
