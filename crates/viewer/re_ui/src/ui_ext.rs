use std::hash::Hash;

use egui::{
    emath::Rot2, pos2, Align2, CollapsingResponse, Color32, NumExt, Rangef, Rect, Vec2, Widget,
};

use crate::{
    design_tokens, icons,
    list_item::{self, LabelContent, ListItem},
    DesignTokens, Icon, LabelStyle, SUCCESS_COLOR,
};

static FULL_SPAN_TAG: &str = "rerun_full_span";

fn error_label_bg_color(fg_color: Color32) -> Color32 {
    fg_color.gamma_multiply(0.35)
}

/// success, warning, error…
fn notification_label(
    ui: &mut egui::Ui,
    fg_color: Color32,
    icon: &str,
    visible_text: &str,
    full_text: &str,
) -> egui::Response {
    egui::Frame::none()
        .stroke((1.0, fg_color))
        .fill(error_label_bg_color(fg_color))
        .rounding(4.0)
        .inner_margin(3.0)
        .outer_margin(1.0) // Needed because we set clip_rect_margin. TODO(emilk): https://github.com/emilk/egui/issues/4019
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.colored_label(fg_color, icon);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let response = ui.strong(visible_text).on_hover_ui(|ui| {
                    if visible_text != full_text {
                        ui.label(full_text);
                        ui.add_space(8.0);
                    }
                    ui.label("Click to copy text.");
                });
                if response.clicked() {
                    ui.ctx().copy_text(full_text.to_owned());
                };
            });
        })
        .response
}

/// Rerun custom extensions to [`egui::Ui`].
pub trait UiExt {
    fn ui(&self) -> &egui::Ui;
    fn ui_mut(&mut self) -> &mut egui::Ui;

    /// Shows a success label with a large border.
    ///
    /// If you don't want a border, use [`crate::ContextExt::success_text`].
    fn success_label(&mut self, success_text: impl Into<String>) -> egui::Response {
        let ui = self.ui_mut();
        let success_text = success_text.into();
        notification_label(ui, SUCCESS_COLOR, "✅", &success_text, &success_text)
    }

    /// Shows a warning label with a large border.
    ///
    /// If you don't want a border, use [`crate::ContextExt::warning_text`].
    fn warning_label(&mut self, warning_text: impl Into<String>) -> egui::Response {
        let ui = self.ui_mut();
        let warning_text = warning_text.into();
        notification_label(
            ui,
            ui.style().visuals.warn_fg_color,
            "⚠",
            &warning_text,
            &warning_text,
        )
    }

    /// Shows a small error label with the given text on hover and copies the text to the clipboard on click with a large border.
    ///
    /// This has a large border! If you don't want a border, use [`crate::ContextExt::error_text`].
    fn error_with_details_on_hover(&mut self, error_text: impl Into<String>) -> egui::Response {
        let ui = self.ui_mut();
        notification_label(
            ui,
            ui.style().visuals.error_fg_color,
            "⚠",
            "Error",
            &error_text.into(),
        )
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
        let ui = self.ui_mut();
        let error_text = error_text.into();
        notification_label(
            ui,
            ui.style().visuals.error_fg_color,
            "⚠",
            &error_text,
            &error_text,
        )
    }

    fn small_icon_button(&mut self, icon: &Icon) -> egui::Response {
        let widget = self.small_icon_button_widget(icon);
        self.ui_mut().add(widget)
    }

    fn small_icon_button_widget<'a>(&self, icon: &'a Icon) -> egui::Button<'a> {
        // TODO(emilk): change color and size on hover
        egui::Button::image(
            icon.as_image()
                .fit_to_exact_size(DesignTokens::small_icon_size())
                .tint(self.ui().visuals().widgets.inactive.fg_stroke.color),
        )
    }

    /// Adds a non-interactive, optionally tinted small icon.
    ///
    /// Uses [`DesignTokens::small_icon_size`]. Returns the rect where the icon was painted.
    fn small_icon(&mut self, icon: &Icon, tint: Option<egui::Color32>) -> egui::Rect {
        let ui = self.ui_mut();
        let (_, rect) = ui.allocate_space(DesignTokens::small_icon_size());
        let mut image = icon.as_image();
        if let Some(tint) = tint {
            image = image.tint(tint);
        }
        image.paint_at(ui, rect);

        rect
    }

    fn medium_icon_toggle_button(&mut self, icon: &Icon, selected: &mut bool) -> egui::Response {
        let size_points = egui::Vec2::splat(16.0); // TODO(emilk): get from design tokens

        let tint = if *selected {
            self.ui().visuals().widgets.inactive.fg_stroke.color
        } else {
            self.ui().visuals().widgets.noninteractive.fg_stroke.color
        };
        let mut response = self
            .ui_mut()
            .add(egui::ImageButton::new(icon.as_image().fit_to_exact_size(size_points)).tint(tint));
        if response.clicked() {
            *selected = !*selected;
            response.mark_changed();
        }
        response
    }

    fn large_button_impl(
        &mut self,
        icon: &Icon,
        bg_fill: Option<Color32>,
        tint: Option<Color32>,
    ) -> egui::Response {
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

        let button_size = Vec2::splat(22.0);
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

            icon.as_image().tint(tint).paint_at(ui, image_rect);
        }

        ui.set_style(prev_style);

        response
    }

    fn re_checkbox(
        &mut self,
        selected: &mut bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.checkbox_indeterminate(selected, text, false)
    }

    #[allow(clippy::disallowed_types)]
    fn checkbox_indeterminate(
        &mut self,
        selected: &mut bool,
        text: impl Into<egui::WidgetText>,
        indeterminate: bool,
    ) -> egui::Response {
        self.ui_mut()
            .scope(|ui| {
                ui.visuals_mut().widgets.hovered.expansion = 0.0;
                ui.visuals_mut().widgets.active.expansion = 0.0;
                ui.visuals_mut().widgets.open.expansion = 0.0;

                egui::Checkbox::new(selected, text)
                    .indeterminate(indeterminate)
                    .ui(ui)
            })
            .inner
    }

    #[allow(clippy::disallowed_methods)]
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
            self.small_icon_button(&icons::VISIBLE)
        } else {
            self.small_icon_button(&icons::INVISIBLE)
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
                painter.round_to_pixel(rect.center().y),
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
        vertical_offset: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        let ui = self.ui();

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

                        crate::list_item::list_item_scope(ui, popup_id, |ui| {
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
                })
            });

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || widget_response.clicked_elsewhere() {
            ui.memory_mut(|mem| mem.close_popup());
        }
        ret
    }

    // TODO(ab): this used to be used for inner margin, after registering full span range in panels.
    // It's highly likely that all these use are now redundant.
    fn panel_content<R>(&mut self, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Frame {
            inner_margin: DesignTokens::panel_margin(),
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
        let ui = self.ui_mut();

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), DesignTokens::title_bar_height()),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // draw horizontal separator lines
                let rect = egui::Rect::from_x_y_ranges(
                    ui.full_span(),
                    ui.available_rect_before_wrap().y_range(),
                );
                let hline_stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;

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
    /// this is equivalent to [`Self::collapsing_header`]. If `collapsing` is `false`, the content
    /// is displayed under a static, non-collapsible header.
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
    fn paint_collapsing_triangle(
        &self,
        openness: f32,
        center: egui::Pos2,
        visuals: &egui::style::WidgetVisuals,
    ) {
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

        self.ui()
            .painter()
            .line(points, (1.0, visuals.fg_stroke.color));
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
    #[allow(clippy::unused_self)]
    fn selection_grid(&self, id: &str) -> egui::Grid {
        // Spread rows a bit to make it easier to see the groupings
        let spacing = egui::vec2(8.0, 16.0);
        egui::Grid::new(id).num_columns(2).spacing(spacing)
    }

    /// Draws a shadow into the given rect with the shadow direction given from dark to light
    fn draw_shadow_line(&self, rect: Rect, direction: egui::Direction) {
        let color_dark = design_tokens().shadow_gradient_dark_start;
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

    /// Convenience function to create a [`list_item::ListItem`].
    #[allow(clippy::unused_self)]
    fn list_item(&self) -> list_item::ListItem {
        list_item::ListItem::new()
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
                |ui| list_item::list_item_scope(ui, id, children_ui),
            )
            .body_response
            .map(|r| r.inner)
    }

    /// Convenience function to create a [`crate::SectionCollapsingHeader`].
    #[allow(clippy::unused_self)]
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

        let icon_width_plus_padding =
            DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding();

        let mut desired_size =
            total_extra + galley.size() + egui::vec2(icon_width_plus_padding, 0.0);
        desired_size.y = desired_size
            .y
            .at_least(ui.spacing().interact_size.y)
            .at_least(DesignTokens::small_icon_size().y);
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
            let image_size = DesignTokens::small_icon_size();
            let image_rect = egui::Rect::from_min_size(
                ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x.ceil(),
                    (rect.center().y - 0.5 * DesignTokens::small_icon_size().y).ceil(),
                )),
                image_size,
            );

            // TODO(emilk, andreas): change color and size on hover
            let tint = ui.visuals().widgets.inactive.fg_stroke.color;
            icon.as_image().tint(tint).paint_at(ui, image_rect);

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x = image_rect.max.x + DesignTokens::text_to_icon_padding();
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

    /// Paints a time cursor for indicating the time on a time axis along x.
    fn paint_time_cursor(
        &self,
        painter: &egui::Painter,
        response: &egui::Response,
        x: f32,
        y: Rangef,
    ) {
        let ui = self.ui();
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
            let fg_fill_on = egui::Color32::from_rgba_premultiplied(0, 128, 255, 255);
            let fg_fill = fg_fill_off.lerp_to_gamma(fg_fill_on, how_on);
            let bg_fill_off = visuals.text_color();

            let rounding = 0.5 * expanded_rect.height();
            ui.painter()
                .rect_filled(expanded_rect, rounding, bg_fill_off);
            let circle_x = egui::lerp(
                (expanded_rect.left() + rounding)..=(expanded_rect.right() - rounding),
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
    fn re_hyperlink(
        &mut self,
        text: impl Into<egui::WidgetText>,
        url: impl ToString,
    ) -> egui::Response {
        let ui = self.ui_mut();
        let response = ListItem::new()
            .show_flat(
                ui,
                LabelContent::new(text).with_icon(&crate::icons::EXTERNAL_LINK),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if response.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        response
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

    fn help_hover_button(&mut self) -> egui::Response {
        self.ui_mut().add(
            egui::Label::new("❓")
                .sense(egui::Sense::click()) // sensing clicks also gives hover effect
                .selectable(false),
        )
    }

    /// Show some markdown
    fn markdown_ui(&mut self, markdown: &str) {
        use parking_lot::Mutex;
        use std::sync::Arc;

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
    ) {
        // TODO(emilk): make the button itself a `ListItem2`
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected_text)
            .show_ui(self.ui_mut(), |ui| {
                list_item::list_item_scope(ui, "inner_scope", |ui| {
                    content(ui);
                });
            });
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
    fn selectable_toggle<R>(&mut self, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ui = self.ui_mut();

        // ensure cursor is on an integer value, otherwise we get weird optical alignment of the text
        //TODO(ab): fix when https://github.com/emilk/egui/issues/4928 is resolved
        ui.add_space(-ui.cursor().min.y.fract());

        egui::Frame {
            inner_margin: egui::Margin::same(3.0),
            stroke: design_tokens().bottom_bar_stroke,
            rounding: ui.visuals().widgets.hovered.rounding + egui::Rounding::same(3.0),
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
        .inner
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
