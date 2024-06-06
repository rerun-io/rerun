//! Core list item functionality.

use egui::{NumExt, Response, Shape, Ui};

use crate::list_item::{ContentContext, DesiredWidth, LayoutInfoStack, ListItemContent};
use crate::{DesignTokens, UiExt as _};

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
    pub draggable: bool,
    pub drag_target: bool,
    pub force_hovered: bool,
    pub collapse_openness: Option<f32>,
    height: f32,
}

impl Default for ListItem {
    fn default() -> Self {
        Self {
            interactive: true,
            selected: false,
            draggable: false,
            drag_target: false,
            force_hovered: false,
            collapse_openness: None,
            height: DesignTokens::list_item_height(),
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

    /// Set the item height.
    ///
    /// The default is provided by [`DesignTokens::list_item_height`] and is suitable for hierarchical
    /// lists.
    #[inline]
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
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
    /// The `id` should be globally unique!
    /// You can use `ui.make_persistent_id(…)` for that.
    ///
    /// *Important*: must be called while nested in a [`super::list_item_scope`].
    pub fn show_hierarchical_with_children<R>(
        mut self,
        ui: &mut Ui,
        id: egui::Id,
        default_open: bool,
        content: impl ListItemContent,
        add_childrens: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ShowCollapsingResponse<R> {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            default_open,
        );

        // enable collapsing arrow
        self.collapse_openness = Some(state.openness(ui.ctx()));

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
                ui.spacing_mut().indent =
                    DesignTokens::small_icon_size().x + DesignTokens::text_to_icon_padding();
                state.show_body_indented(&response.response, ui, |ui| add_childrens(ui))
            })
            .inner;

        ShowCollapsingResponse {
            item_response: response.response,
            body_response,
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
            draggable,
            drag_target,
            force_hovered,
            collapse_openness,
            height,
        } = self;

        let collapse_extra = if collapse_openness.is_some() {
            DesignTokens::collapsing_triangle_area().x + DesignTokens::text_to_icon_padding()
        } else {
            0.0
        };

        let desired_width = match content.desired_width(ui) {
            // // content will use all available width
            // None => ui.available_width().at_least(extra_indent + collapse_extra),
            // // content will use the required width
            // Some(desired_width) => extra_indent + collapse_extra + desired_width,
            DesiredWidth::Exact(width) => extra_indent + collapse_extra + width,
            DesiredWidth::AtLeast(width) => ui
                .available_width()
                .at_least(extra_indent + collapse_extra + width),
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

        // override_hover should not affect the returned response
        let mut style_response = response.clone();
        if force_hovered {
            style_response.contains_pointer = true;
            style_response.hovered = true;
        }

        let mut collapse_response = None;

        let visuals = ui.style().interact_selectable(&style_response, selected);

        let background_frame = ui.painter().add(egui::Shape::Noop);

        // Draw collapsing triangle
        if let Some(openness) = collapse_openness {
            let triangle_pos = ui.painter().round_pos_to_pixels(egui::pos2(
                rect.min.x,
                rect.center().y - 0.5 * DesignTokens::collapsing_triangle_area().y,
            ));
            let triangle_rect =
                egui::Rect::from_min_size(triangle_pos, DesignTokens::collapsing_triangle_area());
            let triangle_response = ui.interact(
                triangle_rect.expand(3.0), // make it easier to click
                id.unwrap_or(ui.id()).with("collapsing_triangle"),
                egui::Sense::click(),
            );
            ui.paint_collapsing_triangle(
                openness,
                triangle_rect.center(),
                ui.style().interact(&triangle_response),
            );
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
        };
        content.ui(ui, &content_ctx);

        if ui.is_rect_visible(bg_rect) {
            // Ensure the background highlight is drawn over round pixel coordinates. Otherwise,
            // there could be artifact between consecutive highlighted items when drawn on
            // fractional pixels.
            let bg_rect_to_paint = ui.painter().round_rect_to_pixels(bg_rect);

            // Draw background on interaction.
            if drag_target {
                ui.painter().set(
                    background_frame,
                    Shape::rect_stroke(
                        bg_rect_to_paint,
                        0.0,
                        (1.0, ui.visuals().selection.bg_fill),
                    ),
                );
            } else {
                let bg_fill = if !response.hovered() && ui.rect_contains_pointer(bg_rect) {
                    // if some part of the content is active and hovered, our background should
                    // become dimmer
                    Some(visuals.bg_fill)
                } else if selected
                    || style_response.hovered()
                    || style_response.highlighted()
                    || style_response.has_focus()
                {
                    Some(visuals.weak_bg_fill)
                } else {
                    None
                };

                if let Some(bg_fill) = bg_fill {
                    ui.painter().set(
                        background_frame,
                        Shape::rect_filled(bg_rect_to_paint, 0.0, bg_fill),
                    );
                }
            }
        }

        ListItemResponse {
            response,
            collapse_response,
        }
    }
}
