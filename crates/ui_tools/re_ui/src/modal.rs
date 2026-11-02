use eframe::emath::{NumExt as _, Vec2};
use egui::{Frame, ModalResponse};

use crate::context_ext::ContextExt as _;
use crate::{DesignTokens, UiExt as _};

/// Helper object to handle a [`ModalWrapper`] window.
///
/// A [`ModalWrapper`] is typically held only so long as it is displayed, so it's typically stored in an
/// [`Option`]. This helper object handles that for your.
///
/// Usage:
/// ```
/// # use re_ui::modal::{ModalWrapper, ModalHandler};
///
/// # egui::__run_test_ui(|ui| {
/// let mut modal_handler = ModalHandler::default();
///
/// if ui.button("Open").clicked() {
///     modal_handler.open();
/// }
///
/// modal_handler.ui(ui.ctx(), || ModalWrapper::new("Modal Window"), |ui| {
///     ui.label("Modal content");
/// });
/// # });
/// ```
pub struct ModalHandler {
    modal: Option<ModalWrapper>,
    animation_started_at: Option<f64>,
    allow_escape: bool,
    should_open: bool,
    should_close: bool,
}

impl Default for ModalHandler {
    fn default() -> Self {
        Self {
            modal: None,
            animation_started_at: None,
            allow_escape: true,
            should_open: false,
            should_close: false,
        }
    }
}

impl ModalHandler {
    /// Allow the user to close the modal by interacting with the backdrop and/or pressing escape.
    pub fn allow_escape(mut self, v: bool) -> Self {
        self.allow_escape = v;
        self
    }

    /// Open the modal the next time the [`ModalHandler::ui`] method is called.
    pub fn open(&mut self) {
        self.should_open = true;
    }

    /// Close the modal the next time the [`ModalHandler::ui`] method is called.
    pub fn close(&mut self) {
        self.should_close = true;
    }

    /// Draw the modal window, creating/destroying it as required.
    pub fn ui<R>(
        &mut self,
        ctx: &egui::Context,
        make_modal: impl FnOnce() -> ModalWrapper,
        content_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        if self.modal.is_none() && self.should_open {
            self.modal = Some(make_modal());
            self.animation_started_at = Some(ctx.input(|input| input.time));
            self.should_open = false;
        }

        if let Some(modal) = &mut self.modal {
            let animation_started_at = *self
                .animation_started_at
                .get_or_insert_with(|| ctx.input(|input| input.time));
            let scale = crate::motion::scale_for_opened_at(ctx, animation_started_at);
            let response = modal.ui_with_scale(ctx, scale, content_ui);

            if self.should_close || (self.allow_escape && response.should_close()) {
                self.modal = None;
                self.animation_started_at = None;
                self.should_close = false;
            }

            Some(response.inner)
        } else {
            None
        }
    }

    /// Whether the modal is currently open.
    pub fn is_open(&self) -> bool {
        self.modal.is_some()
    }
}

/// Show a modal window with Rerun style using [`egui::Modal`].
///
/// The modal sets the clip rect such as to allow full-span highlighting behavior (e.g. with
/// [`crate::list_item::ListItem`]). Consider using [`crate::UiExt::full_span_separator`] to draw a
/// separator that spans the full width of the modal instead of the usual [`egui::Ui::separator`]
/// method.
///
/// Note that [`ModalWrapper`] are typically used via the [`ModalHandler`] helper object to reduce
/// boilerplate.
pub struct ModalWrapper {
    title: String,
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    default_height: Option<f32>,
    full_span_content: bool,
    set_side_margins: bool,
    scrollable: egui::Vec2b,
}

impl ModalWrapper {
    /// Create a new modal with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            default_height: None,
            full_span_content: false,
            set_side_margins: true,
            scrollable: false.into(),
        }
    }

    /// Set the minimum width of the modal window.
    #[inline]
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width);
        self
    }

    /// Set the minimum height of the modal window.
    #[inline]
    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = Some(min_height);
        self
    }

    /// Set the maximum width of the modal window.
    ///
    /// The modal is never wider than the window.
    #[inline]
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    /// Set the maximum height of the modal window.
    ///
    /// The modal is never taller than the window, so `f32::INFINITY` makes it as tall as the window
    /// allows.
    #[inline]
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Set the default height of the modal window.
    #[inline]
    pub fn default_height(mut self, default_height: f32) -> Self {
        self.default_height = Some(default_height);
        self
    }

    /// Configure the content area of the modal for full span highlighting.
    ///
    /// This includes:
    /// - setting the vertical spacing to 0.0
    /// - removing any padding at the bottom of the area
    ///
    /// In this mode, the user code is responsible for adding spacing between items.
    #[inline]
    pub fn full_span_content(mut self, full_span_content: bool) -> Self {
        self.full_span_content = full_span_content;
        self
    }

    /// Configure whether the side margin should be set.
    ///
    /// In general, the side margin should be set for a better layout. It may be useful to not set
    /// them if the client code wants to setup a custom scroll area, which should be outside of the
    /// side margins.
    #[inline]
    pub fn set_side_margin(mut self, set_side_margins: bool) -> Self {
        self.set_side_margins = set_side_margins;
        self
    }

    /// Enclose the contents in a scroll area.
    #[inline]
    pub fn scrollable(mut self, scrollable: impl Into<egui::Vec2b>) -> Self {
        self.scrollable = scrollable.into();
        self
    }

    /// Show the modal window.
    ///
    /// Typically called by [`ModalHandler::ui`].
    pub fn ui<R>(
        &self,
        ctx: &egui::Context,
        content_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ModalResponse<R> {
        self.ui_with_scale(ctx, 1.0, content_ui)
    }

    fn ui_with_scale<R>(
        &self,
        ctx: &egui::Context,
        scale: f32,
        content_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ModalResponse<R> {
        let tokens = ctx.tokens();
        let id = egui::Id::unique(&self.title);
        let mut area = egui::Modal::default_area(id).accessible_name(&self.title);
        if let Some(default_height) = self.default_height {
            area = area.default_height(default_height);
        }

        egui::Modal::new(id.with("modal"))
            .frame(Frame::new())
            .backdrop_color(tokens.modal_backdrop_color)
            .area(area)
            .show(ctx, |ui| {
                prevent_shrinking(ui);

                // Only the surface may be scaled, not the full-screen backdrop sharing the modal's
                // layer, so the opening animation draws into a sublayer. Pointer hit-testing only
                // sees the modal's own layer, so scroll areas only receive the mouse wheel once
                // the content is back in that layer.
                let is_animating = scale < 1.0;
                let content_layer_id = if is_animating {
                    let content_layer_id = self.content_layer_id();
                    ctx.set_sublayer(ui.layer_id(), content_layer_id);
                    content_layer_id
                } else {
                    ui.layer_id()
                };

                let (inner, surface_rect) = ui
                    .scope_builder(egui::UiBuilder::new().layer_id(content_layer_id), |ui| {
                        let response = egui::Frame {
                            fill: ctx.global_style().visuals.panel_fill,
                            corner_radius: tokens.window_corner_radius().into(),
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            ui.set_clip_rect(ui.max_rect());

                            let item_spacing_y = ui.spacing().item_spacing.y;
                            ui.spacing_mut().item_spacing.y = 0.0;

                            /// The space kept free between the modal and each edge of the window.
                            const WINDOW_MARGIN: f32 = 20.0;

                            // The modal never grows past the window, also when the window shrinks
                            // while the modal is open.
                            let window_size = (ui.content_rect().size()
                                - Vec2::splat(2.0 * WINDOW_MARGIN))
                            .max(Vec2::ZERO);

                            if let Some(min_width) = self.min_width {
                                ui.set_min_width(min_width.min(window_size.x));
                            }
                            ui.set_max_width(
                                self.max_width
                                    .unwrap_or(tokens.default_modal_width)
                                    .min(window_size.x),
                            );

                            if let Some(min_height) = self.min_height {
                                ui.set_min_height(min_height.min(window_size.y));
                            }
                            if let Some(max_height) = self.max_height {
                                ui.set_max_height(max_height.min(window_size.y));
                            }

                            //
                            // Title bar
                            //

                            view_padding_frame(
                                tokens,
                                &ViewPaddingFrameParams {
                                    left_and_right: true,
                                    top: true,
                                    bottom: false,
                                },
                            )
                            .show(ui, |ui| {
                                Self::title_bar(ui, &self.title);
                                ui.add_space(tokens.view_padding() as f32);
                                ui.full_span_separator();
                            });

                            //
                            // Inner content
                            //

                            let wrapped_content_ui = |ui: &mut egui::Ui| -> R {
                                // We always have side margin, but these must happen _inside_ the scroll area
                                // (if any). Otherwise, the scroll bar is not snug with the right border and
                                // may interfere with the action buttons of `ListItem`s.
                                view_padding_frame(
                                    tokens,
                                    &ViewPaddingFrameParams {
                                        left_and_right: self.set_side_margins,
                                        top: false,
                                        bottom: false,
                                    },
                                )
                                .show(ui, |ui| {
                                    if self.full_span_content {
                                        // no further spacing for the content UI
                                        content_ui(ui)
                                    } else {
                                        // we must restore vertical spacing and add view padding at the bottom
                                        ui.add_space(item_spacing_y);

                                        view_padding_frame(
                                            tokens,
                                            &ViewPaddingFrameParams {
                                                left_and_right: false,
                                                top: false,
                                                bottom: true,
                                            },
                                        )
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.y = item_spacing_y;
                                            content_ui(ui)
                                        })
                                        .inner
                                    }
                                })
                                .inner
                            };

                            //
                            // Optional scroll area
                            //

                            if self.scrollable.any() {
                                // Make the modal size less jumpy and work around https://github.com/emilk/egui/issues/5138
                                let max_height = 0.85 * ui.content_rect().height();
                                let min_height =
                                    0.3 * ui.content_rect().height().at_most(max_height);

                                egui::ScrollArea::new(self.scrollable)
                                    .min_scrolled_height(max_height)
                                    .max_height(max_height)
                                    .show(ui, |ui| {
                                        let res = wrapped_content_ui(ui);

                                        if ui.min_rect().height() < min_height {
                                            ui.add_space(min_height - ui.min_rect().height());
                                        }

                                        res
                                    })
                                    .inner
                            } else {
                                wrapped_content_ui(ui)
                            }
                        });

                        (response.inner, response.response.rect)
                    })
                    .inner;

                if is_animating {
                    ctx.transform_layer_shapes_after_rounding(
                        content_layer_id,
                        crate::motion::scale_transform(surface_rect.center(), scale),
                    );
                }

                inner
            })
    }

    fn content_layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique(&self.title).with("content"),
        )
    }

    /// Display a title bar in our own style.
    fn title_bar(ui: &mut egui::Ui, title: &str) {
        // The row is as tall as the close button, so the title centers on it rather than the
        // button hanging below a shorter row.
        egui::Sides::new()
            .height(crate::Size::inherited(ui).height())
            .spacing(16.0)
            .shrink_left()
            .truncate()
            .show(
                ui,
                |ui| {
                    ui.strong(title);
                },
                |ui| {
                    if ui
                        .small_icon_button(&crate::icons::CLOSE, "Close")
                        .clicked()
                    {
                        ui.close();
                    }
                },
            );
    }
}

struct ViewPaddingFrameParams {
    left_and_right: bool,
    top: bool,
    bottom: bool,
}

/// Utility to produce a [`egui::Frame`] with padding on some sides.
#[inline]
fn view_padding_frame(tokens: &DesignTokens, params: &ViewPaddingFrameParams) -> egui::Frame {
    let ViewPaddingFrameParams {
        left_and_right,
        top,
        bottom,
    } = *params;
    egui::Frame {
        inner_margin: egui::Margin {
            left: if left_and_right {
                tokens.view_padding()
            } else {
                0
            },
            right: if left_and_right {
                tokens.view_padding()
            } else {
                0
            },
            top: if top { tokens.view_padding() } else { 0 },
            bottom: if bottom { tokens.view_padding() } else { 0 },
        },
        ..Default::default()
    }
}

/// Prevent a UI from shrinking if the content size changes.
///
/// Should be called at the beginning of the [`egui::Ui`], before any other content is added.
/// Will reset if the screen size changes.
pub fn prevent_shrinking(ui: &mut egui::Ui) {
    // The Uis response at this point will conveniently contain last frame's rect
    let last_rect = ui.response().rect;

    #[expect(clippy::useless_let_if_seq)]
    let mut screen_size = ui.content_rect().size();
    if ui.is_sizing_pass() {
        // On the very first frame, there will be a sizing pass and the max_rect that frame might
        // be bigger than necessary. We don't want to lock to that size, so we need to ignore it.
        // To ignore, we can't just return here, but we need to skip the next frame as well.
        // The easiest way to do this is force a reset next frame, by changing the screen size:
        screen_size = Vec2::ZERO;
    }

    let id = ui.make_persistent_id("prevent_shrinking");
    let screen_size_changed = ui.data_mut(|d| {
        let last_screen_size = d.get_temp_mut_or_insert_with(id, || screen_size);
        let changed = *last_screen_size != screen_size;
        *last_screen_size = screen_size;
        changed
    });

    if last_rect.is_positive() && !screen_size_changed {
        let min_size = ui.min_size();
        // Set the min size, respecting the current min size.
        ui.set_min_size(egui::Vec2::max(last_rect.size(), min_size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_animation_only_transforms_the_surface_layer() {
        let ctx = egui::Context::default();
        let modal = ModalWrapper::new("Animated modal").min_width(240.0);
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 700.0));

        let render = || {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |ui| {
                    modal.ui_with_scale(ui.ctx(), 0.5, |ui| {
                        ui.add_sized(egui::vec2(200.0, 160.0), egui::Button::new("Action"));
                    });
                },
            );
            output.textures_delta.clear();
            output
        };

        // The first frame is an invisible sizing pass.
        render();
        let output = render();

        assert!(
            output
                .shapes
                .iter()
                .any(|clipped| clipped.transform_after_tessellation.scaling == 0.5),
            "the modal surface should be scaled"
        );
        let backdrop = output
            .shapes
            .iter()
            .find(|clipped| {
                matches!(&clipped.shape, egui::Shape::Rect(rect) if rect.rect == screen_rect)
            })
            .expect("the modal should paint a full-screen backdrop");
        assert_eq!(
            backdrop.transform_after_tessellation,
            egui::emath::TSTransform::IDENTITY,
            "the full-screen backdrop must not be scaled"
        );
        assert_eq!(
            ctx.layer_transform_to_global(modal.content_layer_id()),
            None,
            "the animation must not move the surface's pixel grid or input coordinates"
        );
    }
}
