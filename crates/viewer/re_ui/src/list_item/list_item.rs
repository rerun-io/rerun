//! Core list item functionality.

use crate::list_item::{ContentContext, DesiredWidth, LayoutInfoStack, ListItemContent};
use crate::{design_tokens, DesignTokens, Scale, UiExt as _};
use egui::emath::GuiRounding as _;
use egui::style::Widgets;
use egui::{Color32, NumExt as _, Response, Shape, Ui};

struct ListItemResponse {
    /// Response of the whole [`ListItem`]
    response: Response,

    /// Response from the collapse-triangle button, if any.
    collapse_response: Option<Response>,
}

/// Responses returned by [`ListItem::show_hierarchical_with_children`].
pub struct ShowCollapsingResponse<R> {
    /// Response from the item itself.
    pub item_response: Response,

    /// Response from the body, if it was displayed.
    pub body_response: Option<egui::InnerResponse<R>>,

    /// 0.0 if fully closed, 1.0 if fully open, and something in-between while animating.
    pub openness: f32,
}

/// Content-generic list item.
///
/// The following features are supported:
/// - Flat or collapsible hierarchical lists.
/// - Full-span background highlighting.
/// - Interactive or not.
/// - Support for drag and drop with [`crate::drag_and_drop`].
///
/// Besides these core features, [`ListItem`] delegates all content to the [`ListItemContent`]
/// implementations, such as [`super::LabelContent`] and [`super::PropertyContent`].
///
/// Usage example can be found in `re_ui_example`.

#[derive(Debug, Clone)]
pub struct ListItem {
    pub interactive: bool,
    pub selected: bool,
    pub active: bool,
    pub draggable: bool,
    pub drag_target: bool,
    pub force_hovered: bool,
    force_background: Option<egui::Color32>,
    pub collapse_openness: Option<f32>,
    height: f32,
    y_offset: f32,
    render_offscreen: bool,
}

impl Default for ListItem {
    fn default() -> Self {
        Self {
            interactive: true,
            selected: false,
            active: false,
            draggable: false,
            drag_target: false,
            force_hovered: false,
            force_background: None,
            collapse_openness: None,
            height: DesignTokens::list_item_height(),
            y_offset: 0.0,
            render_offscreen: true,
        }
    }
}

/// Implemented after <https://www.figma.com/design/04eHlTWW361rIs3YesfTJo/Data-platform?node-id=813-9806&t=Kofxiju5Tn4DszG2-1>
#[derive(Debug, Clone, Copy)]
pub struct ListVisuals {
    hovered: bool,
    selected: bool,
    active: bool,
}

impl ListVisuals {
    pub fn bg_color(self) -> Option<Color32> {
        if self.selected {
            Some(design_tokens().color_table.blue(Scale::S350))
        } else if self.hovered {
            Some(design_tokens().color_table.gray(Scale::S250))
        } else if self.active {
            Some(design_tokens().color_table.gray(Scale::S200))
        } else {
            None
        }
    }

    pub fn text_color(self) -> Color32 {
        if self.selected {
            design_tokens().color_table.blue(Scale::S800)
        } else if self.active {
            design_tokens().color_table.gray(Scale::S1000)
        } else if self.hovered {
            design_tokens().color_table.gray(Scale::S800)
        } else {
            design_tokens().color_table.gray(Scale::S700)
        }
    }

    pub fn icon_tint(self) -> Color32 {
        if self.selected {
            design_tokens().color_table.blue(Scale::S600)
        } else if self.active {
            design_tokens().color_table.gray(Scale::S800)
        } else if self.hovered {
            design_tokens().color_table.gray(Scale::S600)
        } else {
            design_tokens().color_table.gray(Scale::S500)
        }
    }

    pub fn interactive_icon_tint(self, icon_hovered: bool) -> Color32 {
        if self.selected && icon_hovered {
            design_tokens().color_table.blue(Scale::S800)
        } else if icon_hovered {
            design_tokens().color_table.gray(Scale::S800)
        } else {
            self.icon_tint()
        }
    }

    fn collapse_button_color(self, icon_hovered: bool) -> Color32 {
        if !self.hovered && !self.selected && !self.active && !icon_hovered {
            design_tokens().color_table.gray(Scale::S700)
        } else {
            self.interactive_icon_tint(icon_hovered)
        }
    }

    fn apply_visuals(self, visuals: &mut Widgets) {
        if self.selected {
            visuals.hovered.bg_fill = design_tokens().color_table.blue(Scale::S400);
            visuals.hovered.weak_bg_fill = design_tokens().color_table.blue(Scale::S400);
            visuals.hovered.fg_stroke.color = design_tokens().color_table.blue(Scale::S800);
            visuals.active.bg_fill = design_tokens().color_table.blue(Scale::S450);
            visuals.active.weak_bg_fill = design_tokens().color_table.blue(Scale::S450);
            visuals.active.fg_stroke.color = design_tokens().color_table.blue(Scale::S850);
        }
    }
}

impl ListItem {
    /// Create a new [`ListItem`] with the given label.
    pub fn new() -> Self {
        Self::default()
    }

    /// Can the user click and interact with it?
    ///
    /// Set to `false` for items that only show info, but shouldn't be interactive.
    /// Note: making the list item non-interactive does not necessarily make its content
    /// non-interactive. For example, a non-interactive list item may be used in conjunction with
    /// [`super::PropertyContent`] to build property-like editors.
    #[inline]
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Set the selected state of the item.
    #[inline]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set the active state of the item.
    #[inline]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
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
    /// **Note**: this flag has otherwise no behavioral effect. It's up to the caller to set it when the item is
    /// being hovered (or otherwise selected as drop target) while a drag is in progress.
    #[inline]
    pub fn drop_target_style(mut self, drag_target: bool) -> Self {
        self.drag_target = drag_target;
        self
    }

    /// Override the hovered state even if the item is not actually hovered.
    ///
    /// Used to highlight items representing things that are hovered elsewhere in the UI. Note that
    /// the [`egui::Response`] returned by [`Self::show_flat`], [`Self::show_hierarchical`], and
    /// [`Self::show_hierarchical_with_children`] will still reflect the actual hover state.
    #[inline]
    pub fn force_hovered(mut self, force_hovered: bool) -> Self {
        self.force_hovered = force_hovered;
        self
    }

    /// Override the background color for the item.
    ///
    /// If set, this takes precedence over [`Self::force_hovered`] and any kind of selection/
    /// interaction-driven background handling.
    #[inline]
    pub fn force_background(mut self, force_background: egui::Color32) -> Self {
        self.force_background = Some(force_background);
        self
    }

    /// Set the item height.
    ///
    /// The default is provided by [`DesignTokens::list_item_height`] and is suitable for hierarchical
    /// lists.
    #[inline]
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set the item's vertical offset.
    ///
    /// NOTE: Can only be positive.
    /// Default is 0.0.
    #[inline]
    pub fn with_y_offset(mut self, y_offset: f32) -> Self {
        self.y_offset = y_offset;
        self
    }

    /// Set the item's vertical offset to `DesignTokens::list_header_vertical_offset()`.
    /// For best results, use this with [`super::LabelContent::header`].
    #[inline]
    pub fn header(mut self) -> Self {
        self.y_offset = DesignTokens::list_header_vertical_offset();
        self
    }

    /// Controls whether [`Self`] calls [`ListItemContent::ui`] when the item is not currently
    /// visible.
    ///
    /// Skipping rendering can increase performances for long lists that are mostly out of view, but
    /// this will prevent any side effects from [`ListItemContent::ui`] from occurring. For this
    /// reason, this is an opt-in optimization.
    #[inline]
    pub fn render_offscreen(mut self, render_offscreen: bool) -> Self {
        self.render_offscreen = render_offscreen;
        self
    }

    /// Draw the item as part of a flat list.
    ///
    /// *Important*: must be called while nested in a [`super::list_item_scope`].
    pub fn show_flat<'a>(self, ui: &mut Ui, content: impl ListItemContent + 'a) -> Response {
        // Note: the purpose of the scope is to minimise interferences on subsequent items' id
        ui.scope(|ui| self.ui(ui, None, 0.0, Box::new(content)))
            .inner
            .response
    }

    /// Draw the item as a leaf node from a hierarchical list.
    ///
    /// *Important*: must be called while nested in a [`super::list_item_scope`].
    pub fn show_hierarchical(self, ui: &mut Ui, content: impl ListItemContent) -> Response {
        // Note: the purpose of the scope is to minimise interferences on subsequent items' id
        ui.scope(|ui| {
            self.ui(
                ui,
                None,
                DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding(),
                Box::new(content),
            )
        })
        .inner
        .response
    }

    /// Draw the item as a non-leaf node from a hierarchical list.
    ///
    /// The `id` should be globally unique! You can use `ui.make_persistent_id(…)` for that. The
    /// children content is indented.
    ///
    /// *Important*: must be called while nested in a [`super::list_item_scope`].
    pub fn show_hierarchical_with_children<R>(
        self,
        ui: &mut Ui,
        id: egui::Id,
        default_open: bool,
        content: impl ListItemContent,
        add_children: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ShowCollapsingResponse<R> {
        self.show_hierarchical_with_children_impl(ui, id, default_open, true, content, add_children)
    }

    /// Draw the item with unindented child content.
    ///
    /// This is similar to [`Self::show_hierarchical_with_children`] but without indent. This is
    /// only for special cases such as [`crate::SectionCollapsingHeader`].
    pub fn show_hierarchical_with_children_unindented<R>(
        self,
        ui: &mut Ui,
        id: egui::Id,
        default_open: bool,
        content: impl ListItemContent,
        add_children: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ShowCollapsingResponse<R> {
        self.show_hierarchical_with_children_impl(
            ui,
            id,
            default_open,
            false,
            content,
            add_children,
        )
    }

    fn show_hierarchical_with_children_impl<R>(
        mut self,
        ui: &mut Ui,
        id: egui::Id,
        default_open: bool,
        indented: bool,
        content: impl ListItemContent,
        add_children: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ShowCollapsingResponse<R> {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            default_open,
        );

        // enable collapsing arrow
        let openness = state.openness(ui.ctx());
        self.collapse_openness = Some(openness);

        // Note: the purpose of the scope is to minimise interferences on subsequent items' id
        let response = ui
            .scope(|ui| self.ui(ui, Some(id), 0.0, Box::new(content)))
            .inner;

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
                if indented {
                    ui.spacing_mut().indent =
                        DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding();
                    state.show_body_indented(&response.response, ui, |ui| add_children(ui))
                } else {
                    state.show_body_unindented(ui, |ui| add_children(ui))
                }
            })
            .inner;

        ShowCollapsingResponse {
            item_response: response.response,
            body_response,
            openness,
        }
    }

    fn ui<'a>(
        self,
        ui: &mut Ui,
        id: Option<egui::Id>,
        extra_indent: f32,
        content: Box<dyn ListItemContent + 'a>,
    ) -> ListItemResponse {
        let Self {
            interactive,
            selected,
            active,
            draggable,
            drag_target,
            force_hovered,
            force_background,
            collapse_openness,
            mut height,
            y_offset,
            render_offscreen,
        } = self;

        if y_offset != 0.0 {
            ui.add_space(y_offset);
            height -= y_offset;
        }

        let collapse_extra = if collapse_openness.is_some() {
            DesignTokens::collapsing_triangle_area().x + DesignTokens::text_to_icon_padding()
        } else {
            0.0
        };

        let desired_width = match content.desired_width(ui) {
            DesiredWidth::Exact(width) => extra_indent + collapse_extra + width,
            DesiredWidth::AtLeast(width) => {
                let total_width = extra_indent + collapse_extra + width;
                if ui.is_sizing_pass() {
                    // In the sizing pass we try to be as small as possible.
                    // egui will then use the maximum width from the sizing pass
                    // as the max width in all following frames.
                    total_width
                } else {
                    // Use as much space as we are given (i.e. fill up the full width of the ui).
                    ui.available_width().at_least(total_width)
                }
            }
        };

        let desired_size = egui::vec2(desired_width, height);

        let sense = if !interactive {
            egui::Sense::hover()
        } else if draggable {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::click()
        };

        let (allocated_id, mut rect) = ui.allocate_space(desired_size);
        rect.min.x += extra_indent;

        // We use the state set by ListItemContainer to determine how far the background should
        // extend.
        let layout_info = LayoutInfoStack::top(ui.ctx());
        let bg_rect = egui::Rect::from_x_y_ranges(ui.full_span(), rect.y_range());

        // Record the max allocated width.
        layout_info.register_max_item_width(ui.ctx(), rect.right() - layout_info.left_x);

        // We want to be able to select/hover the item across its full span, so we interact over the
        // entire background rect. But…
        let mut response = ui.interact(bg_rect, allocated_id, sense);

        // …we must not "leak" rects that span beyond `ui.available_width()` (which is typically
        // the case for `bg_rect`), because that can have unwanted side effect. For example, it
        // could trigger `show_body_indented` (in `Self::show_hierarchical_with_children`) to
        // allocate past the available width.
        response.rect = rect;

        let should_render = render_offscreen || ui.is_rect_visible(rect);
        if !should_render {
            return ListItemResponse {
                response,
                collapse_response: None,
            };
        }

        // override_hover should not affect the returned response
        let mut style_response = response.clone();
        if force_hovered {
            style_response.flags |= egui::response::Flags::CONTAINS_POINTER;
            style_response.flags |= egui::response::Flags::HOVERED;
        }

        let visuals = ListVisuals {
            hovered: (style_response.hovered() || style_response.contains_pointer())
                && interactive
                && !drag_target
                && !egui::DragAndDrop::has_any_payload(ui.ctx()),
            selected,
            active,
        };

        let mut collapse_response = None;

        let background_frame = ui.painter().add(egui::Shape::Noop);

        // Draw collapsing triangle
        if let Some(openness) = collapse_openness {
            let triangle_pos = egui::pos2(
                rect.min.x,
                rect.center().y - 0.5 * DesignTokens::collapsing_triangle_area().y,
            )
            .round_to_pixels(ui.pixels_per_point());
            let triangle_rect =
                egui::Rect::from_min_size(triangle_pos, DesignTokens::collapsing_triangle_area());
            let triangle_response = ui.interact(
                triangle_rect.expand(3.0), // make it easier to click
                id.unwrap_or(ui.id()).with("collapsing_triangle"),
                egui::Sense::click(),
            );

            let color = visuals.collapse_button_color(triangle_response.hovered());

            ui.paint_collapsing_triangle(openness, triangle_rect.center(), color);
            collapse_response = Some(triangle_response);
        }

        // Draw content
        let mut content_rect = rect;
        if collapse_openness.is_some() {
            content_rect.min.x += extra_indent + collapse_extra;
        }

        let content_ctx = ContentContext {
            rect: content_rect,
            bg_rect,
            response: &style_response,
            list_item: &self,
            layout_info,
            visuals,
        };

        let prev_widgets = ui.style_mut().visuals.widgets.clone();
        visuals.apply_visuals(&mut ui.style_mut().visuals.widgets);
        content.ui(ui, &content_ctx);
        ui.style_mut().visuals.widgets = prev_widgets;

        if ui.is_rect_visible(bg_rect) {
            // Ensure the background highlight is drawn over round pixel coordinates. Otherwise,
            // there could be artifact between consecutive highlighted items when drawn on
            // fractional pixels.
            let bg_rect_to_paint = bg_rect.round_to_pixels(ui.pixels_per_point());

            if drag_target {
                let stroke = crate::design_tokens().drop_target_container_stroke();
                ui.painter().set(
                    background_frame,
                    Shape::rect_stroke(
                        bg_rect_to_paint.shrink(stroke.width),
                        0.0,
                        stroke,
                        egui::StrokeKind::Inside,
                    ),
                );
            }

            if let Some(bg_fill) = force_background.or_else(|| visuals.bg_color()) {
                ui.painter().set(
                    background_frame,
                    Shape::rect_filled(bg_rect_to_paint, 0.0, bg_fill),
                );
            }
        }

        ListItemResponse {
            response,
            collapse_response,
        }
    }
}
