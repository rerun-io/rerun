use crate::{Icon, LabelStyle, ReUi};
use egui::epaint::text::TextWrapping;
use egui::{Align, Align2, Response, Shape, Ui};
use std::default::Default;

struct ListItemResponse {
    /// Response of the whole [`ListItem`]
    response: Response,

    /// Response from the collapse-triangle button, if any.
    collapse_response: Option<Response>,
}

/// Responses returned by [`ListItem::show_collapsing`].
pub struct ShowCollapsingResponse<R> {
    /// Response from the item itself.
    pub item_response: Response,

    /// Response from the body, if it was displayed.
    pub body_response: Option<egui::InnerResponse<R>>,
}

/// Specification of how the width of the [`ListItem`] must be allocated.
#[derive(Default, Clone, Copy, Debug)]
pub enum WidthAllocationMode {
    /// Allocate the full available width.
    ///
    /// This mode is useful for fixed-width container, but should be avoided for dynamically-sized
    /// containers as they will immediately grow to their max width.
    ///
    /// Examples of resulting layouts:
    /// ```text
    ///                      ◀──────available width────▶
    ///
    ///                      ┌─────────────────────────┐
    ///              normal: │▼ □ label                │
    ///                      └─────────────────────────┘
    ///                      ┌─────────────────────────┐
    ///             hovered: │▼ □ label             ■ ■│
    ///                      └─────────────────────────┘
    ///                      ┌──────────────────────────┐
    ///  normal, long label: │▼ □ a very, very long lab…│
    ///                      └──────────────────────────┘
    ///                      ┌──────────────────────────┐
    /// hovered, long label: │▼ □ a very, very long… ■ ■│
    ///                      └──────────────────────────┘
    /// ```
    /// The allocated size is always the same, and the label is truncated depending on the available
    /// space, which is further reduced whenever buttons are displayed.
    #[default]
    Available,

    /// Allocate the width needed for the text and icon(s) (if any).
    ///
    /// This mode doesn't account for buttons (if any). If buttons are enabled, the label will get
    /// truncated when they are displayed.
    ///
    /// Examples of resulting layouts:
    /// ```text
    ///                      ┌─────────┐
    ///              normal: │▼ □ label│
    ///                      └─────────┘
    ///                      ┌─────────┐
    ///             hovered: │▼ □ … ■ ■│
    ///                      └─────────┘
    ///                      ┌───────────────────────────┐
    ///  normal, long label: │▼ □ a very, very long label│
    ///                      └───────────────────────────┘
    ///                      ┌───────────────────────────┐
    /// hovered, long label: │▼ □ a very, very long … ■ ■│
    ///                      └───────────────────────────┘
    /// ```
    Compact,
}

/// Generic widget for use in lists.
///
/// Layout:
/// ```text
/// ┌───┬────────────────────────────────────────────────────────────┬───┐
/// │   │┌──────┐ ┌──────┐                           ┌──────┐┌──────┐│   │
/// │   ││  __  │ │      │                           │      ││      ││   │
/// │   ││  \/  │ │ icon │  label                    │ btns ││ btns ││   │
/// │   ││      │ │      │                           │      ││      ││   │
/// │   │└──────┘ └──────┘                           └──────┘└──────┘│   │
/// └───┴────────────────────────────────────────────────────────────┴───┘
///     ◀───────────── allocated width (used for layout) ───────────▶
/// ◀────────────── clip rectangle (used for highlighting) ─────────────▶
/// ```
///
/// Features:
/// - selectable
/// - full span highlighting based on clip rectangle
/// - optional icon
/// - optional on-hover buttons on the right
/// - optional collapsing behavior for trees
///
/// This widget relies on the clip rectangle to be properly set as it use it for the shape if its
/// background highlighting. This has a significant impact on the hierarchy of the UI. This is
/// typically how things should be laid out:
///
/// ```text
/// Panel (no margin, set the clip rectangle)
/// └── ScrollArea (no margin)
///     └── Frame (with inner margin)
///         └── ListItem
/// ```
///
/// See [`ReUi::panel_content`] for an helper to build the [`egui::Frame`] with proper margins.
#[allow(clippy::type_complexity)]
pub struct ListItem<'a> {
    text: egui::WidgetText,
    re_ui: &'a ReUi,
    active: bool,
    selected: bool,
    draggable: bool,
    drag_target: bool,
    subdued: bool,
    weak: bool,
    italics: bool,
    label_style: crate::LabelStyle,
    force_hovered: bool,
    collapse_openness: Option<f32>,
    height: f32,
    width_allocation_mode: WidthAllocationMode,
    icon_fn: Option<Box<dyn FnOnce(&ReUi, &egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a>>,
    buttons_fn: Option<Box<dyn FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a>>,
}

impl<'a> ListItem<'a> {
    /// Create a new [`ListItem`] with the given label.
    pub fn new(re_ui: &'a ReUi, text: impl Into<egui::WidgetText>) -> Self {
        Self {
            text: text.into(),
            re_ui,
            active: true,
            selected: false,
            draggable: false,
            drag_target: false,
            subdued: false,
            weak: false,
            italics: false,
            label_style: crate::LabelStyle::default(),
            force_hovered: false,
            collapse_openness: None,
            height: ReUi::list_item_height(),
            width_allocation_mode: Default::default(),
            icon_fn: None,
            buttons_fn: None,
        }
    }

    /// Set the active state the item.
    #[inline]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set the selected state of the item.
    #[inline]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Make the item draggable.
    #[inline]
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Highlight the item as the current drop target.
    ///
    /// Use this while dragging, to highlight which container will receive the drop at any given time.
    /// **Note**: this flag has otherwise no behavioural effect. It's up to the caller to set it when the item is
    /// being hovered (or otherwise selected as drop target) while a drag is in progress.
    #[inline]
    pub fn drop_target_style(mut self, drag_target: bool) -> Self {
        self.drag_target = drag_target;
        self
    }

    /// Set the subdued state of the item.
    ///
    /// Note: takes precedence over [`Self::weak`] if set.
    // TODO(ab): this is a hack to implement the behavior of the blueprint tree UI, where active
    // widget are displayed in a subdued state (container, hidden space views/entities). One
    // slightly more correct way would be to override the color using a (color, index) pair
    // related to the design system table.
    #[inline]
    pub fn subdued(mut self, subdued: bool) -> Self {
        self.subdued = subdued;
        self
    }

    /// Set the weak state of the item.
    ///
    /// Note: [`Self::subdued`] takes precedence if set.
    // TODO(ab): should use design token instead
    #[inline]
    pub fn weak(mut self, weak: bool) -> Self {
        self.weak = weak;
        self
    }

    /// Render text in italic.
    // TODO(ab): should use design token instead
    #[inline]
    pub fn italics(mut self, italics: bool) -> Self {
        self.italics = italics;
        self
    }

    /// Style the label for an unnamed items.
    ///
    /// The styling is applied on top of to [`Self::weak`] and [`Self::subdued`]. It also implies [`Self::italics`].
    // TODO(ab): should use design token instead
    #[inline]
    pub fn label_style(mut self, style: crate::LabelStyle) -> Self {
        self.label_style = style;
        self
    }

    /// Override the hovered state even if the item is not actually hovered.
    ///
    /// Used to highlight items representing things that are hovered elsewhere in the UI. Note that
    /// the [`egui::Response`] returned by [`Self::show`] and ]`Self::show_collapsing`] will still
    /// reflect the actual hover state.
    #[inline]
    pub fn force_hovered(mut self, force_hovered: bool) -> Self {
        self.force_hovered = force_hovered;
        self
    }

    /// Set the item height.
    ///
    /// The default is provided by [`ReUi::list_item_height`] and is suitable for hierarchical
    /// lists.
    #[inline]
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set the width allocation mode.
    #[inline]
    pub fn width_allocation_mode(mut self, mode: WidthAllocationMode) -> Self {
        self.width_allocation_mode = mode;
        self
    }

    /// Provide an [`Icon`] to be displayed on the left of the item.
    #[inline]
    pub fn with_icon(self, icon: &'a Icon) -> Self {
        self.with_icon_fn(|_, ui, rect, visuals| {
            let tint = visuals.fg_stroke.color;
            icon.as_image().tint(tint).paint_at(ui, rect);
        })
    }

    /// Provide a custom closure to draw an icon on the left of the item.
    #[inline]
    pub fn with_icon_fn(
        mut self,
        icon_fn: impl FnOnce(&ReUi, &egui::Ui, egui::Rect, egui::style::WidgetVisuals) + 'a,
    ) -> Self {
        self.icon_fn = Some(Box::new(icon_fn));
        self
    }

    /// Provide a closure to display on-hover buttons on the right of the item.
    ///
    /// Notes:
    /// - If buttons are used, the item will allocate the full available width of the parent. If the
    ///   enclosing UI adapts to the childrens width, it will unnecessarily grow. If buttons aren't
    ///   used, the item will only allocate the width needed for the text and icons if any.
    /// - A right to left layout is used, so the right-most button must be added first.
    #[inline]
    pub fn with_buttons(
        mut self,
        buttons: impl FnOnce(&ReUi, &mut egui::Ui) -> egui::Response + 'a,
    ) -> Self {
        self.buttons_fn = Some(Box::new(buttons));
        self
    }

    /// Draw the item.
    pub fn show(self, ui: &mut Ui) -> Response {
        // Note: the purpose of the scope is to minimise interferences on subsequent items' id
        ui.scope(|ui| self.ui(ui, None)).inner.response
    }

    /// Draw the item as a collapsing header.
    pub fn show_collapsing<R>(
        mut self,
        ui: &mut Ui,
        id: egui::Id,
        default_open: bool,
        add_body: impl FnOnce(&ReUi, &mut egui::Ui) -> R,
    ) -> ShowCollapsingResponse<R> {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            default_open,
        );

        // enable collapsing arrow
        self.collapse_openness = Some(state.openness(ui.ctx()));

        let re_ui = self.re_ui;
        // Note: the purpose of the scope is to minimise interferences on subsequent items' id
        let response = ui.scope(|ui| self.ui(ui, Some(id))).inner;

        if let Some(collapse_response) = response.collapse_response {
            if collapse_response.clicked() {
                state.toggle(ui);
            }
        }
        if response.response.double_clicked() {
            state.toggle(ui);
        }

        let body_response = ui
            .scope(|ui| {
                ui.spacing_mut().indent = ReUi::small_icon_size().x + ReUi::text_to_icon_padding();
                state.show_body_indented(&response.response, ui, |ui| add_body(re_ui, ui))
            })
            .inner;

        ShowCollapsingResponse {
            item_response: response.response,
            body_response,
        }
    }

    fn ui(mut self, ui: &mut Ui, id: Option<egui::Id>) -> ListItemResponse {
        let collapse_extra = if self.collapse_openness.is_some() {
            ReUi::collapsing_triangle_area().x + ReUi::text_to_icon_padding()
        } else {
            0.0
        };
        let icon_extra = if self.icon_fn.is_some() {
            ReUi::small_icon_size().x + ReUi::text_to_icon_padding()
        } else {
            0.0
        };

        match self.label_style {
            LabelStyle::Normal => {}
            LabelStyle::Unnamed => {
                self.italics = true;
            }
        }

        if self.italics {
            self.text = self.text.italics();
        }

        /// Compute the "ideal" desired width of the item, accounting for text and icon(s) (but not
        /// buttons).
        fn icons_and_label_width(
            ui: &egui::Ui,
            item: &ListItem<'_>,
            collapse_extra: f32,
            icon_extra: f32,
        ) -> f32 {
            let layout_job = item.text.clone().into_layout_job(
                ui.style(),
                egui::FontSelection::Default,
                Align::LEFT,
            );
            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            let text_width = galley.size().x;

            // The `ceil()` is needed to avoid some rounding errors which leads to text being
            // truncated even though we allocated enough space.
            (collapse_extra + icon_extra + text_width).ceil()
        }

        let desired_width = match self.width_allocation_mode {
            WidthAllocationMode::Available => ui.available_width(),
            WidthAllocationMode::Compact => {
                icons_and_label_width(ui, &self, collapse_extra, icon_extra)
            }
        };

        let desired_size = egui::vec2(desired_width, self.height);
        let (rect, mut response) = ui.allocate_at_least(
            desired_size,
            if self.draggable {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            },
        );

        // compute the full-span background rect
        let mut bg_rect = rect;
        bg_rect.extend_with_x(ui.clip_rect().right());
        bg_rect.extend_with_x(ui.clip_rect().left());

        // we want to be able to select/hover the item across its full span, so we sense that and
        // update the response accordingly.
        let full_span_response = ui.interact(bg_rect, response.id, egui::Sense::click());
        response.clicked = full_span_response.clicked;
        response.contains_pointer = full_span_response.contains_pointer;
        response.hovered = full_span_response.hovered;

        // override_hover should not affect the returned response
        let mut style_response = response.clone();
        if self.force_hovered {
            style_response.contains_pointer = true;
            style_response.hovered = true;
        }

        let mut collapse_response = None;

        if ui.is_rect_visible(bg_rect) {
            let mut visuals = if self.active {
                ui.style()
                    .interact_selectable(&style_response, self.selected)
            } else {
                ui.visuals().widgets.inactive
            };

            // TODO(ab): use design tokens instead
            if self.weak {
                visuals.fg_stroke.color = ui.visuals().weak_text_color();
            } else if self.subdued {
                visuals.fg_stroke.color = visuals.fg_stroke.color.gamma_multiply(0.5);
            }

            let background_frame = ui.painter().add(egui::Shape::Noop);

            // Draw collapsing triangle
            if let Some(openness) = self.collapse_openness {
                let triangle_pos = ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x,
                    rect.center().y - 0.5 * ReUi::collapsing_triangle_area().y,
                ));
                let triangle_rect =
                    egui::Rect::from_min_size(triangle_pos, ReUi::collapsing_triangle_area());
                let triangle_response = ui.interact(
                    triangle_rect.expand(3.0), // make it easier to click
                    id.unwrap_or(ui.id()).with("collapsing_triangle"),
                    egui::Sense::click(),
                );
                ReUi::paint_collapsing_triangle(
                    ui,
                    openness,
                    triangle_rect.center(),
                    &triangle_response,
                );
                collapse_response = Some(triangle_response);
            }

            // Draw icon
            if let Some(icon_fn) = self.icon_fn {
                let icon_pos = ui.painter().round_pos_to_pixels(egui::pos2(
                    rect.min.x + collapse_extra,
                    rect.center().y - 0.5 * ReUi::small_icon_size().y,
                ));
                let icon_rect = egui::Rect::from_min_size(icon_pos, ReUi::small_icon_size());
                icon_fn(self.re_ui, ui, icon_rect, visuals);
            }

            // Handle buttons
            // Note: We should be able to just use `response.hovered()` here, which only returns `true` if no drag is in
            // progress. Due to the response merging we do above, this breaks though. This is why we do an explicit
            // rectangle and drag payload check.
            //TODO(ab): refactor responses to address that.
            let should_show_buttons = self.active
                && ui.rect_contains_pointer(rect)
                && !egui::DragAndDrop::has_any_payload(ui.ctx());
            let button_response = if should_show_buttons {
                if let Some(buttons) = self.buttons_fn {
                    let mut ui =
                        ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::Center));
                    Some(buttons(self.re_ui, &mut ui))
                } else {
                    None
                }
            } else {
                None
            };

            // Draw text next to the icon.
            let mut text_rect = rect;
            text_rect.min.x += collapse_extra + icon_extra;
            if let Some(button_response) = &button_response {
                text_rect.max.x -= button_response.rect.width() + ReUi::text_to_icon_padding();
            }

            match self.label_style {
                LabelStyle::Normal => {}
                LabelStyle::Unnamed => {
                    self.text = self.text.color(visuals.fg_stroke.color.gamma_multiply(0.5));
                }
            }

            let mut layout_job =
                self.text
                    .into_layout_job(ui.style(), egui::FontSelection::Default, Align::LEFT);
            layout_job.wrap = TextWrapping::truncate_at_width(text_rect.width());

            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            // this happens here to avoid cloning the text
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    self.selected,
                    galley.text(),
                )
            });

            let text_pos = Align2::LEFT_CENTER
                .align_size_within_rect(galley.size(), text_rect)
                .min;

            ui.painter().galley(text_pos, galley, visuals.text_color());

            // Draw background on interaction.
            if self.drag_target {
                ui.painter().set(
                    background_frame,
                    Shape::rect_stroke(bg_rect, 0.0, (1.0, ui.visuals().selection.bg_fill)),
                );
            } else {
                let bg_fill = if button_response.map_or(false, |r| r.hovered()) {
                    Some(visuals.bg_fill)
                } else if self.selected
                    || style_response.hovered()
                    || style_response.highlighted()
                    || style_response.has_focus()
                {
                    Some(visuals.weak_bg_fill)
                } else {
                    None
                };

                if let Some(bg_fill) = bg_fill {
                    ui.painter()
                        .set(background_frame, Shape::rect_filled(bg_rect, 0.0, bg_fill));
                }
            }
        }

        ListItemResponse {
            response,
            collapse_response,
        }
    }
}
